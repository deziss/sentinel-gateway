use crate::error::{DbError, DbResult};
use crate::models::team::{CreateTeam, Team, TeamMember};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct TeamRepository {
    pool: DbPool,
}

impl TeamRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateTeam) -> DbResult<Team> {
        let id = Uuid::new_v4();
        let team = sqlx::query_as::<_, Team>(
            r#"
            INSERT INTO teams (id, tenant_id, name, slug, description)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, tenant_id, name, slug, description, settings, is_active, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.name)
        .bind(input.slug)
        .bind(input.description)
        .fetch_one(&self.pool)
        .await?;
        Ok(team)
    }

    pub async fn find_by_id(&self, id: Uuid, tenant_id: Uuid) -> DbResult<Team> {
        sqlx::query_as::<_, Team>(
            "SELECT id, tenant_id, name, slug, description, settings, is_active, created_at, updated_at
             FROM teams WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Team {id}")))
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<Team>> {
        let teams = sqlx::query_as::<_, Team>(
            "SELECT id, tenant_id, name, slug, description, settings, is_active, created_at, updated_at
             FROM teams WHERE tenant_id = $1 AND is_active = true ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(teams)
    }

    pub async fn delete(&self, id: Uuid, tenant_id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE teams SET is_active = false, updated_at = NOW() WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Team members ──────────────────────────────────────────────────────

    pub async fn add_member(&self, team_id: Uuid, user_id: Uuid, role: &str) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (team_id, user_id) DO UPDATE SET role = $3",
        )
        .bind(team_id)
        .bind(user_id)
        .bind(role)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
            .bind(team_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_members(&self, team_id: Uuid) -> DbResult<Vec<TeamMember>> {
        let members = sqlx::query_as::<_, TeamMember>(
            "SELECT team_id, user_id, role, joined_at FROM team_members WHERE team_id = $1 ORDER BY joined_at",
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(members)
    }

    pub async fn teams_for_user(&self, user_id: Uuid) -> DbResult<Vec<Team>> {
        let teams = sqlx::query_as::<_, Team>(
            r#"
            SELECT t.id, t.tenant_id, t.name, t.slug, t.description, t.settings, t.is_active, t.created_at, t.updated_at
            FROM teams t
            JOIN team_members tm ON tm.team_id = t.id
            WHERE tm.user_id = $1 AND t.is_active = true
            ORDER BY t.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(teams)
    }
}
