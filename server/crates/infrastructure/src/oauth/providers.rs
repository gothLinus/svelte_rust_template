use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Google,
    Apple,
    Github,
    Microsoft,
}

pub(crate) struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub scope: &'static str,
    pub extra_authorize_params: &'static [(&'static str, &'static str)],
}

impl Provider {
    pub const ALL: [Self; 4] = [Self::Google, Self::Apple, Self::Github, Self::Microsoft];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Apple => "apple",
            Self::Github => "github",
            Self::Microsoft => "microsoft",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Apple => "Apple",
            Self::Github => "GitHub",
            Self::Microsoft => "Microsoft",
        }
    }

    /// The variable that turns the provider on. Every variable name of a provider is spelled only
    /// here and in [`Provider::extra_vars`].
    pub const fn client_id_var(self) -> &'static str {
        match self {
            Self::Google => "OAUTH_GOOGLE_CLIENT_ID",
            Self::Apple => "OAUTH_APPLE_CLIENT_ID",
            Self::Github => "OAUTH_GITHUB_CLIENT_ID",
            Self::Microsoft => "OAUTH_MICROSOFT_CLIENT_ID",
        }
    }

    /// The client secret's variable; Apple signs its secret with a private key instead.
    pub const fn client_secret_var(self) -> &'static str {
        match self {
            Self::Google => "OAUTH_GOOGLE_CLIENT_SECRET",
            Self::Apple => "OAUTH_APPLE_PRIVATE_KEY",
            Self::Github => "OAUTH_GITHUB_CLIENT_SECRET",
            Self::Microsoft => "OAUTH_MICROSOFT_CLIENT_SECRET",
        }
    }

    pub const fn extra_vars(self) -> &'static [&'static str] {
        match self {
            Self::Apple => &["OAUTH_APPLE_TEAM_ID", "OAUTH_APPLE_KEY_ID"],
            Self::Microsoft => &["OAUTH_MICROSOFT_TENANT"],
            Self::Google | Self::Github => &[],
        }
    }

    pub const fn is_openid(self) -> bool {
        matches!(self, Self::Google | Self::Apple | Self::Microsoft)
    }

    pub(crate) fn endpoints(self, microsoft_tenant: Option<&str>) -> Endpoints {
        let (authorize, token, scope, extra): (String, String, _, &[_]) = match self {
            Self::Google => (
                "https://accounts.google.com/o/oauth2/v2/auth".into(),
                "https://oauth2.googleapis.com/token".into(),
                "openid email profile",
                &[("prompt", "select_account")],
            ),
            // Apple posts the result back as a form when the name or email scope is asked for; the
            // callback accepts both.
            Self::Apple => (
                "https://appleid.apple.com/auth/authorize".into(),
                "https://appleid.apple.com/auth/token".into(),
                "name email",
                &[("response_mode", "form_post")],
            ),
            Self::Github => (
                "https://github.com/login/oauth/authorize".into(),
                "https://github.com/login/oauth/access_token".into(),
                "read:user user:email",
                &[],
            ),
            Self::Microsoft => {
                let tenant = microsoft_tenant.unwrap_or("common");
                (
                    format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"),
                    format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"),
                    "openid email profile",
                    &[("prompt", "select_account")],
                )
            }
        };
        Endpoints {
            authorize,
            token,
            scope,
            extra_authorize_params: extra,
        }
    }
}

impl Display for Provider {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
