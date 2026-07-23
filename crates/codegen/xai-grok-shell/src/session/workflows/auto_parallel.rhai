let meta = #{
    name: "auto-parallel",
    description: "Automatic parallel task orchestrator — phân tích task, split thành subtasks độc lập, dispatch song song, tổng hợp kết quả, kiểm tra conflict.",
    when_to_use: "Use when you have a complex task that can be split into multiple independent subtasks — e.g., fixing bugs in multiple files, refactoring multiple modules, reviewing multiple components, researching multiple topics.",
    phases: [
        #{ title: "Phân tích", detail: "Split task thành subtasks độc lập" },
        #{ title: "Thi công", detail: "Parallel dispatch N subagents" },
        #{ title: "Tổng hợp", detail: "Gom kết quả, kiểm tra conflict" },
    ],
};

// ── Schemas ──────────────────────────────────────────────────────────

let analysis_schema = #{
    "type": "object", "required": ["subtasks"],
    "properties": #{
        "subtasks": #{ "type": "array", "maxItems": 8, "items": #{
            "type": "object", "required": ["name", "description", "capability_mode"],
            "properties": #{
                "name": #{ "type": "string", "description": "Short label for this subtask" },
                "description": #{ "type": "string", "description": "Detailed instructions for the subagent" },
                "capability_mode": #{ "type": "string", "enum": ["read-only", "read-write", "execute", "all"] },
                "needs_isolation": #{ "type": "boolean", "description": "Whether this subtask needs an isolated worktree (true if it edits files)" },
            },
        }},
        "rationale": #{ "type": "string", "description": "Why this split makes sense" },
    },
};

let synthesis_schema = #{
    "type": "object", "required": ["summary", "results"],
    "properties": #{
        "summary": #{ "type": "string", "description": "Overall summary of what was accomplished" },
        "results": #{ "type": "array", "items": #{
            "type": "object", "required": ["name", "status"],
            "properties": #{
                "name": #{ "type": "string" },
                "status": #{ "type": "string", "enum": ["success", "failed", "skipped"] },
                "output_summary": #{ "type": "string" },
            },
        }},
        "conflicts": #{ "type": "array", "items": #{ "type": "string" }, "description": "Any conflicts detected between subtasks" },
    },
};

// ── Guard ────────────────────────────────────────────────────────────

let task = if args == () { () } else { if args.task != () { args.task } else { args.objective } };
if task == () {
    pause("verification", "Pass `args.task` — mô tả task cần thực hiện. Ví dụ: /workflow auto-parallel task=\"Fix bugs in parser.rs, lexer.rs, codegen.rs\"");
}

let max_agents = if args == () || args.max_agents == () { 4 } else { args.max_agents };
let isolation = if args == () || args.isolation == () { true } else { args.isolation };

log("━━━ Auto Parallel Orchestrator ━━━");
log("Task: " + task);
log("Max parallel agents: " + max_agents.to_string());
log("Isolation: " + (if isolation { "yes" } else { "no" }));

// ── Phase 1: Analyze task → split into subtasks ─────────────────────

phase("Phân tích");
let analysis = agent(
    "Bạn là chuyên gia phân tích task. Nhiệm vụ: đọc task dưới đây và chia thành các subtasks độc lập.\n\n"
    + "Task: " + task + "\n\n"
    + "QUY TẮC:\n"
    + "1. Chỉ tạo subtask khi task THỰC SỰ có thể split — nếu không, trả về 1 subtask duy nhất\n"
    + "2. Subtasks phải độc lập: không shared state, không dependency lẫn nhau\n"
    + "3. Nếu subtask A cần kết quả của subtask B → gộp chúng lại làm 1\n"
    + "4. Mỗi subtask cần: name (ngắn gọn), description (chi tiết, self-contained), capability_mode\n"
    + "5. capability_mode: 'read-only' (chỉ đọc), 'read-write' (đọc+viết file), 'execute' (chạy lệnh), 'all' (full access)\n"
    + "6. needs_isolation: true nếu subtask edit file (để tránh conflict với subtask khác)\n"
    + "7. Tối đa " + max_agents.to_string() + " subtasks\n"
    + "8. Đọc codebase trước khi trả lời — không trả lời từ memory\n"
    + "9. Trả lời bằng tiếng Việt (trừ code/technical terms)",
    #{ label: "analyzer", capability_mode: "read-only", output_schema: analysis_schema }
);

if analysis == () || !analysis.success {
    pause("verification", "Analysis agent failed. Kiểm tra lại task description và thử lại.");
}

let subtasks = if analysis.output.subtasks == () { [] } else { analysis.output.subtasks };
let n = subtasks.len();
log("→ Phát hiện " + n.to_string() + " subtasks");
if analysis.output.rationale != () && analysis.output.rationale != "" {
    log("Rationale: " + analysis.output.rationale);
}

if n == 0 {
    complete(#{
        summary: "Task quá đơn giản, không cần parallel. Thực hiện tuần tự.",
        total_subtasks: 0,
        successful: 0,
        failed: 0,
        results: [],
        conflicts: [],
        task: task,
    });
}

// ── Phase 2: Execute subtasks in parallel ────────────────────────────

phase("Thi công");
let jobs = [];
for i in 0..n {
    let s = subtasks[i];
    let needs_iso = if s.needs_isolation == () { true } else { s.needs_isolation };
    jobs.push(#{
        prompt: "Bạn là subagent thực hiện subtask sau:\n\n"
            + "━━ " + s.name + " ━━\n"
            + s.description + "\n\n"
            + "YÊU CẦU:\n"
            + "1. Đọc code trước khi làm — không trả lời từ memory\n"
            + "2. Chỉ thay đổi đúng phạm vi subtask này\n"
            + "3. KHÔNG sửa code ngoài phạm vi subtask\n"
            + "4. Báo cáo kết quả chi tiết: đã làm gì, thay đổi file nào, kết quả ra sao\n"
            + "5. Nếu gặp lỗi, mô tả lỗi và nguyên nhân\n"
            + "6. Trả lời bằng tiếng Việt (trừ code output/technical terms)",
        label: s.name,
        capability_mode: s.capability_mode,
        isolation_worktree: if isolation && needs_iso { true } else { false },
    });
}
let results = parallel(jobs);

// ── Phase 3: Synthesize results ──────────────────────────────────────

phase("Tổng hợp");

// Build synthesis prompt from results
let synthesis_prompt = "Tổng hợp kết quả từ " + n.to_string() + " subtasks đã chạy song song:\n\n";
let success_count = 0;
for i in 0..n {
    let r = results[i];
    let status = if r == () || !r.success { "❌ failed" } else { "✅ success" };
    if r != () && r.success { success_count += 1; }
    synthesis_prompt += "### " + subtasks[i].name + " " + status + "\n";
    if r != () && r.success {
        synthesis_prompt += r.output + "\n\n";
    } else {
        let err = if r == () { "No result (parallel slot returned null)" } else { "Subagent returned success=false" };
        synthesis_prompt += err + "\n\n";
    }
}
synthesis_prompt += "─── Kiểm tra ───\n";
synthesis_prompt += "1. Có conflict giữa các subtask không? (cùng sửa 1 file, cùng thay đổi 1 logic)\n";
synthesis_prompt += "2. Tất cả subtask đã hoàn thành đúng không?\n";
synthesis_prompt += "3. Tổng hợp kết quả cuối cùng thành 1 báo cáo ngắn gọn.\n";
synthesis_prompt += "4. Nếu có conflict, đề xuất cách giải quyết.\n";

let synthesis = agent(synthesis_prompt,
    #{ label: "synthesizer", capability_mode: "read-only", output_schema: synthesis_schema }
);

let final_output = if synthesis != () && synthesis.success {
    synthesis.output
} else {
    #{ summary: "Synthesis agent failed. Xem raw results bên dưới.", results: [] }
};

let failed = n - success_count;
log("━━━ Kết quả: " + success_count.to_string() + "/" + n.to_string() + " subtasks thành công ━━━");

complete(#{
    summary: final_output.summary,
    total_subtasks: n,
    successful: success_count,
    failed: failed,
    results: final_output.results,
    conflicts: final_output.conflicts,
    task: task,
});