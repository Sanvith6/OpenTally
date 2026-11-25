use crate::errors::OpenTallyError;
use crate::orders::models::PendingOrdersReport;
use crate::orders::OrdersService;
use sqlx::{Pool, Sqlite};

pub struct OrderOutstandingReportService;

impl OrderOutstandingReportService {
    pub async fn get_pending_purchase_orders(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<PendingOrdersReport, OpenTallyError> {
        OrdersService::get_pending_purchase_orders(pool, company_id, as_of_date).await
    }

    pub async fn get_pending_sales_orders(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<PendingOrdersReport, OpenTallyError> {
        OrdersService::get_pending_sales_orders(pool, company_id, as_of_date).await
    }
}
