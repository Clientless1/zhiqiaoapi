-- Embeddings share token billing with chat, but grant a different capability.
UPDATE provider_model SET capabilities=capabilities || '{"embeddings":true}'::jsonb
WHERE lower(model) LIKE '%embedding%';
INSERT INTO user_package (name,description,family)
SELECT '向量模型','按实际用量计费，仅限向量与 Embedding 模型。','embedding'
WHERE NOT EXISTS (SELECT 1 FROM user_package WHERE family='embedding');
