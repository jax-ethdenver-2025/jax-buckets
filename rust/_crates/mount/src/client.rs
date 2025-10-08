impl ApiClient {
    pub async fn call<T: ApiRequest>(&mut self, request: T) -> Result<T::Response, ApiError> {
        let response = self.client
            .request(request.method(), &request.url(self.remote.as_str()))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ApiError::StatusCode(
                response.status(),
                response.text().await?,
            ));
        }

        Ok(response.json().await?)
    }
} 