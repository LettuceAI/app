DELETE FROM memory_inherited_summary_source_messages
WHERE EXISTS (
    SELECT 1 FROM memory_inherited_summaries summary
    JOIN conversation_branches branch
      ON branch.conversation_id = summary.conversation_id AND branch.id = summary.branch_id
    WHERE summary.space_id = memory_inherited_summary_source_messages.space_id
      AND branch.parent_branch_id IS NULL
);
DELETE FROM memory_inherited_summaries
WHERE EXISTS (
    SELECT 1 FROM conversation_branches branch
    WHERE branch.conversation_id = memory_inherited_summaries.conversation_id
      AND branch.id = memory_inherited_summaries.branch_id
      AND branch.parent_branch_id IS NULL
);
