use super::{AxumUser, AxumUserStore, OAuthCallbackError, OAuthFlow, OAuthToken};
use crate::{RefreshInitResult, UnmatchedOAuthToken};
pub use oauth2::RefreshToken;
use oauth2::{AuthorizationCode, CsrfToken};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OAuthRefreshCallbackError<StoreError: std::error::Error> {
    #[error(transparent)]
    OAuthCallbackError(#[from] OAuthCallbackError),
    #[error("Expected a login flow, got {0}")]
    UnexpectedFlow(OAuthFlow),
    #[error("Previous token not found")]
    TokenNotFound,
    #[error(transparent)]
    Store(StoreError),
}

#[derive(Debug, Error)]
pub enum OAuthRefreshInitError {
    #[error("Refresh not allowed")]
    NotAllowed,
    #[error("No provider found with name: {0}")]
    ProviderNotFound(String),
    #[error(transparent)]
    ExchangeError(#[from] anyhow::Error),
}

impl<S: AxumUserStore> AxumUser<S> {
    pub async fn oauth_refresh_init(
        self,
        token: S::OAuthToken,
        next: Option<String>,
    ) -> Result<(Self, RefreshInitResult), OAuthRefreshInitError> {
        let provider = self
            .oauth
            .providers
            .get(token.provider_name())
            .ok_or(OAuthRefreshInitError::ProviderNotFound(
                token.provider_name().to_string(),
            ))
            .cloned()?;

        if let Some(refresh_token) = token.refresh_token() {
            let res = provider
                .exchange_refresh_token(
                    provider.name(),
                    &self.redirect_uri(self.oauth.refresh_path.clone(), provider.name()),
                    &RefreshToken::new(refresh_token.to_string()),
                )
                .await?;

            let _ = self.store.oauth_update_token(token, res).await;

            Ok((self, RefreshInitResult::Ok))
        } else {
            let path = self.oauth.refresh_path.clone();
            let (new_self, url) = self
                .oauth_init(
                    path,
                    provider,
                    OAuthFlow::Refresh {
                        token_id: token.id(),
                        next,
                    },
                )
                .await;

            Ok((new_self, RefreshInitResult::Redirect(url)))
        }
    }

    pub(crate) async fn oauth_refresh_callback_inner(
        &self,
        unmatched_token: UnmatchedOAuthToken,
        flow: OAuthFlow,
    ) -> Result<Option<String>, OAuthRefreshCallbackError<S::Error>> {
        let OAuthFlow::Refresh { token_id, next } = flow else {
            return Err(OAuthRefreshCallbackError::UnexpectedFlow(flow));
        };

        let Some(old_token) = self
            .store
            .oauth_get_token(token_id)
            .await
            .map_err(OAuthRefreshCallbackError::Store)?
        else {
            return Err(OAuthRefreshCallbackError::TokenNotFound);
        };

        self.store
            .oauth_update_token(old_token, unmatched_token)
            .await
            .map_err(OAuthRefreshCallbackError::Store)?;

        Ok(next)
    }

    pub async fn oauth_refresh_callback(
        &self,
        provider_name: String,
        code: AuthorizationCode,
        state: CsrfToken,
    ) -> Result<Option<String>, OAuthRefreshCallbackError<S::Error>> {
        let (unmatched_token, flow, _provider) = self
            .oauth_callback_inner(
                provider_name.clone(),
                code,
                state,
                self.oauth.refresh_path.clone(),
            )
            .await?;

        self.oauth_refresh_callback_inner(unmatched_token, flow)
            .await
    }
}
