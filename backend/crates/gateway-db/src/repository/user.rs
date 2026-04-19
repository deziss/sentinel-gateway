use crate::error::{DbError, DbResult};
use crate::models::user::{CreateUser, User, UserRole, UserStatus};
use crate::pool::DbPool;
use uuid::Uuid;

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateUser) -> DbResult<User> {
        let id = Uuid::new_v4();
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, tenant_id, email, password_hash, role, status)
            VALUES ($1, $2, $3, $4, $5, 'active')
            RETURNING id, tenant_id, email, password_hash, role, status,
                      mfa_secret, failed_login_attempts, locked_until, last_login_at,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.tenant_id)
        .bind(input.email)
        .bind(input.password_hash)
        .bind(input.role)
        .fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn find_by_id(&self, id: Uuid, tenant_id: Uuid) -> DbResult<User> {
        sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, password_hash, role, status,
                   mfa_secret, failed_login_attempts, locked_until, last_login_at,
                   created_at, updated_at
            FROM users WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("User {id}")))
    }

    pub async fn find_by_email(&self, email: &str, tenant_id: Uuid) -> DbResult<User> {
        sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, password_hash, role, status,
                   mfa_secret, failed_login_attempts, locked_until, last_login_at,
                   created_at, updated_at
            FROM users WHERE email = $1 AND tenant_id = $2
            "#,
        )
        .bind(email)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("User email={email}")))
    }

    pub async fn list_by_tenant(&self, tenant_id: Uuid) -> DbResult<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, password_hash, role, status,
                   mfa_secret, failed_login_attempts, locked_until, last_login_at,
                   created_at, updated_at
            FROM users WHERE tenant_id = $1 ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(users)
    }

    pub async fn increment_failed_attempts(&self, id: Uuid) -> DbResult<i32> {
        let row: (i32,) = sqlx::query_as(
            r#"
            UPDATE users SET failed_login_attempts = failed_login_attempts + 1, updated_at = NOW()
            WHERE id = $1
            RETURNING failed_login_attempts
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn count_by_tenant(&self, tenant_id: Uuid) -> DbResult<i64> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE tenant_id = $1 AND status != 'inactive'",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn update_role(&self, id: Uuid, tenant_id: Uuid, role: UserRole) -> DbResult<User> {
        sqlx::query_as::<_, User>(
            r#"
            UPDATE users SET role = $3, updated_at = NOW()
            WHERE id = $1 AND tenant_id = $2
            RETURNING id, tenant_id, email, password_hash, role, status,
                      mfa_secret, failed_login_attempts, locked_until, last_login_at,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(role)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("User {id}")))
    }

    pub async fn update_status(&self, id: Uuid, tenant_id: Uuid, status: UserStatus) -> DbResult<()> {
        sqlx::query(
            "UPDATE users SET status = $3, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn lock_user(&self, id: Uuid, duration_minutes: i64) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET locked_until = NOW() + make_interval(mins => $2),
                status = 'locked',
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(duration_minutes as f64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn reset_failed_attempts(&self, id: Uuid) -> DbResult<()> {
        sqlx::query(
            "UPDATE users SET failed_login_attempts = 0, locked_until = NULL, last_login_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_password(&self, id: Uuid, password_hash: String) -> DbResult<()> {
        sqlx::query(
            "UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(password_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
