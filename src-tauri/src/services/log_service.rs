use crate::database::{
    ChartDataPoint, DashboardStats, Database, ModelRanking, PaginatedResult, UsageLog,
    UsageLogFilter, UserRanking,
};
use crate::error::AppError;

/// Get paginated usage logs
pub fn get_usage_logs(
    db: &Database,
    filter: &UsageLogFilter,
) -> Result<PaginatedResult<UsageLog>, AppError> {
    db.get_usage_logs(filter)
}

/// Get dashboard statistics
pub fn get_dashboard_stats(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
) -> Result<DashboardStats, AppError> {
    db.get_dashboard_stats(start_time, end_time)
}

/// Get model consumption chart data
pub fn get_model_consumption(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
    granularity: Option<&str>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    db.get_model_consumption(start_time, end_time, granularity)
}

/// Get call trend chart data
pub fn get_call_trend(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
    granularity: Option<&str>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    db.get_call_trend(start_time, end_time, granularity)
}

/// Get model distribution for pie chart
pub fn get_model_distribution(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
) -> Result<Vec<ModelRanking>, AppError> {
    db.get_model_distribution(start_time, end_time)
}

/// Get model ranking
pub fn get_model_ranking(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
) -> Result<Vec<ModelRanking>, AppError> {
    db.get_model_ranking(start_time, end_time)
}

/// Get user ranking
pub fn get_user_ranking(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
) -> Result<Vec<UserRanking>, AppError> {
    db.get_user_ranking(start_time, end_time)
}

/// Get user trend chart data
pub fn get_user_trend(
    db: &Database,
    start_time: Option<i64>,
    end_time: Option<i64>,
    granularity: Option<&str>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    db.get_user_trend(start_time, end_time, granularity)
}
