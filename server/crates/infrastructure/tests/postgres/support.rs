use domain::{
    database::Database,
    rbac::{RbacRepository, RoleName},
    user::{Email, NewUser, PasswordHash, User, UserRepository, Username},
};
use infrastructure::db::{PgExecutor, PostgresDatabase};
use sqlx::{PgPool, Postgres, pool::PoolConnection};

pub type Conn = PgExecutor<PoolConnection<Postgres>>;

pub async fn conn(pool: &PgPool) -> Conn {
    PostgresDatabase::new(pool.clone())
        .connection()
        .await
        .unwrap()
}

pub fn new_user(email: &str) -> NewUser {
    NewUser {
        id: domain::user::UserId::generate(),
        email: Email::parse(email).unwrap(),
        username: Username::parse(email.split('@').next().unwrap()).unwrap(),
        password_hash: Some(PasswordHash::new("$argon2id$fake")),
        email_verified_at: None,
    }
}

pub async fn user(conn: &mut Conn, email: &str) -> User {
    let user = conn.create_user(&new_user(email)).await.unwrap();
    conn.grant_role(user.id(), &RoleName::USER).await.unwrap();
    user
}
