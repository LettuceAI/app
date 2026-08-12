pub fn has_manual_memories(memories: &[String]) -> bool {
    !memories.is_empty()
}

pub fn render_manual_memory_lines(memories: &[String]) -> String {
    memories
        .iter()
        .map(|memory| format!("- {}", memory))
        .collect::<Vec<_>>()
        .join("\n")
}
