use std::sync::Arc;

use crate::{
    Adapters, Context,
    account::AccountService,
    admin::AdminService,
    audit::AuditService,
    auth::{AuthService, ReauthService},
    health::HealthService,
    maintenance::MaintenanceService,
    mfa::MfaService,
    notes::NoteService,
    oauth::OAuthService,
    passkeys::PasskeyService,
    passwordless::PasswordlessService,
};

/// The application's use cases, built once at startup and shared by all requests.
pub struct Services<A: Adapters> {
    pub auth: AuthService<A>,
    pub reauth: ReauthService<A>,
    pub passwordless: PasswordlessService<A>,
    pub passkeys: PasskeyService<A>,
    pub mfa: MfaService<A>,
    pub oauth: OAuthService<A>,
    pub account: AccountService<A>,
    pub admin: AdminService<A>,
    pub audit: AuditService<A>,
    pub notes: NoteService<A>,
    pub health: HealthService<A>,
    pub maintenance: MaintenanceService<A>,
    context: Arc<Context<A>>,
}

impl<A: Adapters> Services<A> {
    pub fn new(context: Context<A>) -> Self {
        let context = Arc::new(context);
        Self {
            auth: AuthService::new(Arc::clone(&context)),
            reauth: ReauthService::new(Arc::clone(&context)),
            passwordless: PasswordlessService::new(Arc::clone(&context)),
            passkeys: PasskeyService::new(Arc::clone(&context)),
            mfa: MfaService::new(Arc::clone(&context)),
            oauth: OAuthService::new(Arc::clone(&context)),
            account: AccountService::new(Arc::clone(&context)),
            admin: AdminService::new(Arc::clone(&context)),
            audit: AuditService::new(Arc::clone(&context)),
            notes: NoteService::new(Arc::clone(&context)),
            health: HealthService::new(Arc::clone(&context)),
            maintenance: MaintenanceService::new(Arc::clone(&context)),
            context,
        }
    }

    pub fn context(&self) -> &Arc<Context<A>> {
        &self.context
    }
}
