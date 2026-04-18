use crate::error::DbResult;
use crate::models::prompt::{CreatePrompt, Prompt, PromptDeployment};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct PromptRepository {
    pool: DbPool,
}

impl PromptRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ── Prompts ──────────────────────────────────────────────────────────────

    /// Create a new prompt version. Auto-increments version if prompt with same
    /// (tenant, name) already exists.
    pub async fn create(&self, input: CreatePrompt) -> DbResult<Prompt> {
        // Get next version number
        let next_version: i32 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(version), 0) + 1
            FROM prompts
            WHERE tenant_id = $1 AND name = $2
            "#,
        )
        .bind(input.tenant_id)
        .bind(&input.name)
        .fetch_one(&self.pool)
        .await?;

        let prompt = sqlx::query_as::<_, Prompt>(
            r#"
            INSERT INTO prompts (
                tenant_id, name, version, content, variables,
                model_prefs, default_model, metadata, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, tenant_id, name, version, content, variables,
                      model_prefs, default_model, metadata, created_by,
                      created_at, updated_at
            "#,
        )
        .bind(input.tenant_id)
        .bind(&input.name)
        .bind(next_version)
        .bind(&input.content)
        .bind(&input.variables)
        .bind(&input.model_prefs)
        .bind(input.default_model.as_deref())
        .bind(&input.metadata)
        .bind(input.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(prompt)
    }

    pub async fn list_names(&self, tenant_id: Uuid) -> DbResult<Vec<String>> {
        let names: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT name FROM prompts WHERE tenant_id = $1 ORDER BY name
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(names)
    }

    pub async fn list_versions(&self, tenant_id: Uuid, name: &str) -> DbResult<Vec<Prompt>> {
        let prompts = sqlx::query_as::<_, Prompt>(
            r#"
            SELECT id, tenant_id, name, version, content, variables,
                   model_prefs, default_model, metadata, created_by,
                   created_at, updated_at
            FROM prompts
            WHERE tenant_id = $1 AND name = $2
            ORDER BY version DESC
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .fetch_all(&self.pool)
        .await?;
        Ok(prompts)
    }

    pub async fn get_version(
        &self,
        tenant_id: Uuid,
        name: &str,
        version: i32,
    ) -> DbResult<Option<Prompt>> {
        let prompt = sqlx::query_as::<_, Prompt>(
            r#"
            SELECT id, tenant_id, name, version, content, variables,
                   model_prefs, default_model, metadata, created_by,
                   created_at, updated_at
            FROM prompts
            WHERE tenant_id = $1 AND name = $2 AND version = $3
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;
        Ok(prompt)
    }

    pub async fn get_latest(&self, tenant_id: Uuid, name: &str) -> DbResult<Option<Prompt>> {
        let prompt = sqlx::query_as::<_, Prompt>(
            r#"
            SELECT id, tenant_id, name, version, content, variables,
                   model_prefs, default_model, metadata, created_by,
                   created_at, updated_at
            FROM prompts
            WHERE tenant_id = $1 AND name = $2
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(prompt)
    }

    pub async fn delete_version(
        &self,
        tenant_id: Uuid,
        name: &str,
        version: i32,
    ) -> DbResult<()> {
        sqlx::query(
            r#"DELETE FROM prompts WHERE tenant_id = $1 AND name = $2 AND version = $3"#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(version)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Deployments ──────────────────────────────────────────────────────────

    /// Deploy a specific version to a label (e.g., "prod", "canary"). Upserts.
    pub async fn deploy(
        &self,
        tenant_id: Uuid,
        name: &str,
        label: &str,
        version: i32,
        deployed_by: Option<Uuid>,
    ) -> DbResult<PromptDeployment> {
        let deployment = sqlx::query_as::<_, PromptDeployment>(
            r#"
            INSERT INTO prompt_deployments (tenant_id, prompt_name, label, version, deployed_by)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (tenant_id, prompt_name, label)
            DO UPDATE SET version = EXCLUDED.version,
                          deployed_by = EXCLUDED.deployed_by,
                          deployed_at = NOW()
            RETURNING id, tenant_id, prompt_name, label, version, deployed_by, deployed_at
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(label)
        .bind(version)
        .bind(deployed_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(deployment)
    }

    pub async fn get_deployment(
        &self,
        tenant_id: Uuid,
        name: &str,
        label: &str,
    ) -> DbResult<Option<PromptDeployment>> {
        let deployment = sqlx::query_as::<_, PromptDeployment>(
            r#"
            SELECT id, tenant_id, prompt_name, label, version, deployed_by, deployed_at
            FROM prompt_deployments
            WHERE tenant_id = $1 AND prompt_name = $2 AND label = $3
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(label)
        .fetch_optional(&self.pool)
        .await?;
        Ok(deployment)
    }

    pub async fn list_deployments(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> DbResult<Vec<PromptDeployment>> {
        let deployments = sqlx::query_as::<_, PromptDeployment>(
            r#"
            SELECT id, tenant_id, prompt_name, label, version, deployed_by, deployed_at
            FROM prompt_deployments
            WHERE tenant_id = $1 AND prompt_name = $2
            ORDER BY label
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .fetch_all(&self.pool)
        .await?;
        Ok(deployments)
    }

    /// Resolve a prompt reference: given (name, label), return the currently deployed
    /// prompt, or fall back to the latest version if no deployment exists for that label.
    /// When label is None, returns the latest version.
    pub async fn resolve(
        &self,
        tenant_id: Uuid,
        name: &str,
        label: Option<&str>,
    ) -> DbResult<Option<Prompt>> {
        if let Some(label) = label {
            if let Some(dep) = self.get_deployment(tenant_id, name, label).await? {
                return self.get_version(tenant_id, name, dep.version).await;
            }
        }
        self.get_latest(tenant_id, name).await
    }

    pub async fn delete_deployment(
        &self,
        tenant_id: Uuid,
        name: &str,
        label: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"DELETE FROM prompt_deployments WHERE tenant_id = $1 AND prompt_name = $2 AND label = $3"#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(label)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
