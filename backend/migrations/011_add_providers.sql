-- Add new LLM provider types
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'qwen';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'xai';
ALTER TYPE backend_provider_type ADD VALUE IF NOT EXISTS 'zai';
