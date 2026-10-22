-- Keep historical migrations immutable. Apply the product rename here so
-- existing NeoGate databases upgrade without replacing their data or checksums.
ALTER TABLE task_upstream DROP CONSTRAINT IF EXISTS task_upstream_task_type_check;
UPDATE task_upstream SET task_type = 'zhiqiaoapi_response' WHERE task_type = 'neogate_response';
ALTER TABLE task_upstream ADD CONSTRAINT task_upstream_task_type_check CHECK (
    task_type IN ('openai_response', 'openai_video', 'audio_transcription',
                  'anthropic_message_batch', 'zhiqiaoapi_response')
);

UPDATE provider SET display_name = 'Anthropic · Claude', name = 'Anthropic Claude', updated_at = now()
WHERE code = 'anthropic' AND display_name IN ('Anthropic 兼容', 'Anthropic 官方', 'Anthropic');

-- These providers use an OpenAI-compatible gateway supplied by the operator.
-- Do not prefill native vendor URLs that require a different API protocol.
INSERT INTO provider (code, display_name, name, sort_order)
VALUES ('blackforestlabs', 'FLUX · 图片（兼容网关）', 'Black Forest Labs', 300),
       ('stability', 'Stable Diffusion · 图片（兼容网关）', 'Stability AI', 310),
       ('kling', '可灵 Kling · 视频（兼容网关）', 'Kling', 320),
       ('runway', 'Runway · 视频（兼容网关）', 'Runway', 330),
       ('elevenlabs', 'ElevenLabs · 语音（兼容网关）', 'ElevenLabs', 340)
ON CONFLICT (code) DO NOTHING;

INSERT INTO provider_model (provider, model, display_name, source, enabled, billing_meter, capabilities)
VALUES
('anthropic', 'claude-sonnet-4-5', 'Claude Sonnet 4.5', 'seed', FALSE, 'token', '{"text":true}'),
('anthropic', 'claude-opus-4-1', 'Claude Opus 4.1', 'seed', FALSE, 'token', '{"text":true}'),
('anthropic', 'claude-haiku-4-5', 'Claude Haiku 4.5', 'seed', FALSE, 'token', '{"text":true}'),
('openai', 'gpt-image-1', 'GPT Image 1', 'seed', FALSE, 'image', '{"image_generation":true}'),
('openai', 'dall-e-3', 'DALL·E 3', 'seed', FALSE, 'image', '{"image_generation":true}'),
('openai', 'sora-2', 'Sora 2', 'seed', FALSE, 'video', '{"video_generation":true}'),
('openai', 'sora-2-pro', 'Sora 2 Pro', 'seed', FALSE, 'video', '{"video_generation":true}'),
('openai', 'tts-1', 'TTS 1', 'seed', FALSE, 'audio', '{"audio_speech":true}'),
('openai', 'tts-1-hd', 'TTS 1 HD', 'seed', FALSE, 'audio', '{"audio_speech":true}'),
('openai', 'gpt-4o-mini-tts', 'GPT-4o mini TTS', 'seed', FALSE, 'audio', '{"audio_speech":true}'),
('blackforestlabs', 'FLUX.1-dev', 'FLUX.1 Dev', 'seed', FALSE, 'image', '{"image_generation":true}'),
('blackforestlabs', 'FLUX.1-schnell', 'FLUX.1 Schnell', 'seed', FALSE, 'image', '{"image_generation":true}'),
('stability', 'stable-diffusion-3.5-large', 'Stable Diffusion 3.5 Large', 'seed', FALSE, 'image', '{"image_generation":true}'),
('kling', 'kling-v1-6', 'Kling 1.6', 'seed', FALSE, 'video', '{"video_generation":true}'),
('kling', 'kling-v2-master', 'Kling 2 Master', 'seed', FALSE, 'video', '{"video_generation":true}'),
('runway', 'gen4-turbo', 'Runway Gen-4 Turbo', 'seed', FALSE, 'video', '{"video_generation":true}'),
('elevenlabs', 'eleven_multilingual_v2', 'Eleven Multilingual v2', 'seed', FALSE, 'audio', '{"audio_speech":true}'),
('elevenlabs', 'eleven_flash_v2_5', 'Eleven Flash v2.5', 'seed', FALSE, 'audio', '{"audio_speech":true}')
ON CONFLICT (provider, model) DO UPDATE SET
    billing_meter = CASE WHEN provider_model.source = 'seed' THEN EXCLUDED.billing_meter ELSE provider_model.billing_meter END,
    capabilities = provider_model.capabilities || EXCLUDED.capabilities;

