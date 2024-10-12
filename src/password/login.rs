use crate::{Allow, AxumUser, AxumUserStore, LoginMethod, User};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PasswordLoginError<T: std::error::Error> {
    #[error("Password login not allowed")]
    NotAllowed,
    #[error("User doesn't exists")]
    NoUser,
    #[error("Wrong password")]
    WrongPassword,
    #[error(transparent)]
    StoreError(#[from] T),
}

impl<S: AxumUserStore> AxumUser<S> {
    #[must_use = "Don't forget to return the auth session as part of the response!"]
    pub async fn password_login(
        self,
        password_id: String,
        password: String,
    ) -> Result<Self, PasswordLoginError<S::Error>> {
        if self.pass.allow_login.as_ref().unwrap_or(&self.allow_login) == &Allow::Never {
            return Err(PasswordLoginError::NotAllowed);
        };

        let user = self
            .store
            .password_login(
                password_id,
                password,
                self.pass
                    .allow_signup
                    .as_ref()
                    .unwrap_or(&self.allow_signup)
                    == &Allow::OnEither,
            )
            .await?;

        Ok(self.log_in(LoginMethod::Password, user.get_id()).await?)
    }
}