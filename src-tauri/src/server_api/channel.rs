//! Channel facade：channel CRUD、模型获取、连接测试等。
//!
//! 调用 `services::channel_service` 中的函数，底层使用 `&Database` 和
//! `Option<&AppEventHandle>`，在所有构建模式下均可用。

use crate::database::dao::PaginatedResult;
use crate::database::{Channel, ModelCatalogMetaInput, ModelInfo};
use crate::error::AppError;
use crate::services::channel_service::{
    self, CreateChannelParams, FetchModelsResult, SaveChannelWithModelsParams,
    SaveChannelWithModelsResult, TestChannelResult, UpdateChannelParams, UpdateResponseMsParams,
};

use super::ServerApi;

impl ServerApi {
    /// 获取所有 channel 列表。
    pub fn list_channels(&self) -> Result<Vec<Channel>, AppError> {
        channel_service::list_channels(&self.state().db)
    }

    /// 获取指定 channel 的模型列表。
    pub async fn fetch_channel_models(
        &self,
        channel_id: String,
    ) -> Result<FetchModelsResult, AppError> {
        channel_service::fetch_models(&self.state().db, channel_id).await
    }

    /// 分页获取 channel 列表。
    pub fn list_channels_paginated(
        &self,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult<Channel>, AppError> {
        channel_service::list_channels_paginated(&self.state().db, page, page_size)
    }

    /// 创建新 channel。
    pub fn create_channel(&self, params: CreateChannelParams) -> Result<Channel, AppError> {
        let channel = channel_service::create_channel(&self.state().db, params)?;
        crate::event::emit(self.app(), "channels-changed");
        Ok(channel)
    }

    /// 更新 channel，同时触发前端事件。
    pub fn update_channel(&self, params: UpdateChannelParams) -> Result<Channel, AppError> {
        channel_service::update_channel(&self.state().db, Some(self.app()), params)
    }

    /// 删除 channel，同时触发前端事件。
    pub fn delete_channel(&self, id: String) -> Result<(), AppError> {
        let result = channel_service::delete_channel(&self.state().db, Some(self.app()), id);
        if result.is_ok() {
            crate::event::emit(self.app(), "entries-changed");
        }
        result
    }

    /// 更新 channel 的响应时间。
    pub fn update_channel_response_ms(
        &self,
        channel_id: &str,
        response_ms: &str,
    ) -> Result<(), AppError> {
        let result = channel_service::update_channel_response_ms(
            &self.state().db,
            UpdateResponseMsParams {
                channel_id: channel_id.to_string(),
                response_ms: response_ms.to_string(),
            },
        );
        if result.is_ok() {
            crate::event::emit(self.app(), "channels-changed");
        }
        result
    }

    /// 同步 channel 的已选模型，并维护由模型派生的 pool entry。
    pub fn select_channel_models(
        &self,
        channel_id: &str,
        model_names: &[String],
        available_models: &[ModelInfo],
        catalog_meta: &[ModelCatalogMetaInput],
    ) -> Result<(), AppError> {
        self.state()
            .db
            .update_channel_models(channel_id, available_models, model_names)?;
        self.state().db.sync_entries_for_channel_with_meta(
            channel_id,
            model_names,
            catalog_meta,
        )?;
        crate::state_version::bump("channel");
        crate::state_version::bump("pool");
        crate::event::emit(self.app(), "channels-changed");
        crate::event::emit(self.app(), "entries-changed");
        Ok(())
    }

    /// 测试指定 channel，并按测试结果更新启用状态/响应时间。
    pub async fn test_channel(&self, channel_id: &str) -> Result<TestChannelResult, AppError> {
        let channel = self.state().db.get_channel(channel_id)?;

        let model = channel
            .selected_models
            .first()
            .or_else(|| channel.available_models.first().map(|m| &m.name))
            .cloned()
            .unwrap_or_else(|| "gpt-3.5-turbo".to_string());

        let result = channel_service::test_channel_chat(
            &channel.base_url,
            &channel.api_key,
            &channel.api_type,
            &model,
        )
        .await;

        if result.success && result.status_code == Some(200) {
            let _ = self
                .state()
                .db
                .update_channel_response_ms(channel_id, &result.latency_ms.to_string());
            let _ = channel_service::update_channel(
                &self.state().db,
                Some(self.app()),
                UpdateChannelParams {
                    id: channel_id.to_string(),
                    name: None,
                    api_type: None,
                    base_url: None,
                    api_key: None,
                    enabled: Some(true),
                    notes: None,
                },
            );
        } else {
            let _ = self.state().db.disable_channel(channel_id);
            crate::state_version::bump("channel");
            crate::event::emit(self.app(), "channels-changed");
        }

        Ok(result)
    }

    /// 一步保存 channel 并同步模型，同时触发前端事件。
    pub fn save_channel_with_models(
        &self,
        params: SaveChannelWithModelsParams,
    ) -> Result<SaveChannelWithModelsResult, AppError> {
        channel_service::save_channel_with_models(&self.state().db, Some(self.app()), params)
    }
}
