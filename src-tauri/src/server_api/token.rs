//! Token facade：access key 的 CRUD 与状态切换。
//!
//! 调用 `services::token_service` 中的函数，底层使用 `&Database`，
//! 在所有构建模式下均可用。

use crate::database::dao::PaginatedResult;
use crate::database::AccessKey;
use crate::error::AppError;

use super::ServerApi;

impl ServerApi {
    /// 获取所有 access key。
    pub fn list_access_keys(&self) -> Result<Vec<AccessKey>, AppError> {
        crate::services::token_service::list_access_keys(&self.state().db)
    }

    /// 分页获取 access key。
    pub fn list_access_keys_paginated(
        &self,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult<AccessKey>, AppError> {
        crate::services::token_service::list_access_keys_paginated(
            &self.state().db,
            page,
            page_size,
        )
    }

    /// 创建新 access key。
    pub fn create_access_key(&self, name: &str) -> Result<AccessKey, AppError> {
        crate::services::token_service::create_access_key(&self.state().db, name)
    }

    /// 删除指定 access key。
    pub fn delete_access_key(&self, id: &str) -> Result<(), AppError> {
        crate::services::token_service::delete_access_key(&self.state().db, id, self.app.as_ref())
    }

    /// 切换 access key 的启用状态。
    pub fn toggle_access_key(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        crate::services::token_service::toggle_access_key(
            &self.state().db,
            id,
            enabled,
            self.app.as_ref(),
        )
    }
}
