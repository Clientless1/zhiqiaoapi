-- Normalize the known historical task-brand variant without rewriting history.
ALTER TABLE task_upstream DROP CONSTRAINT task_upstream_task_type_check;
UPDATE task_upstream SET task_type='zhiqiaoapi_response' WHERE task_type='neogate_response';
ALTER TABLE task_upstream ADD CONSTRAINT task_upstream_task_type_check CHECK (
    task_type IN ('openai_response','openai_video','audio_transcription','anthropic_message_batch','zhiqiaoapi_response')
);
