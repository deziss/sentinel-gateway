-- Provider catalog expansion: add 7 new OpenAI-compatible providers.
-- Enables "~90% of production LLM traffic" coverage per competitive roadmap.
--
-- All new providers share the OpenAI request/response shape, so no adapter
-- translation is needed — only the enum, URL builder, and default endpoint.

ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'mistral';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'cohere';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'deepseek';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'groq';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'together';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'perplexity';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'fireworks';
