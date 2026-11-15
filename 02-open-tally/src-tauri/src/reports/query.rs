use crate::errors::OpenTallyError;
use crate::reports::model::{ReportFilter, ReportMovement, ReportOpeningBalance};
use crate::vouchers::service::execute_read_only_query;
use sqlx::{Pool, Sqlite};

pub struct ReportQueryService;

impl ReportQueryService {
    /// Loads only financially effective voucher lines for one company and period.
    ///
    /// Posted reversing vouchers are included because they carry the compensating
    /// double-entry lines. The original voucher is marked `reversed` by the
    /// reversal transaction and is included as historical activity; cancelled
    /// and draft vouchers are excluded from financial reports.
    pub async fn list_movements(
        pool: &Pool<Sqlite>,
        company_id: &str,
        filter: &ReportFilter,
    ) -> Result<Vec<ReportMovement>, OpenTallyError> {
        let company_id = company_id.to_string();
        let filter = filter.clone();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let mut sql = String::from(
                    "SELECT v.id AS voucher_id, v.voucher_number, v.date AS voucher_date,
                            vt.name AS voucher_type_name, v.status,
                            vl.ledger_id, l.name AS ledger_name, vl.line_number,
                            vl.entry_type, vl.amount, vl.narration
                     FROM vouchers v
                     JOIN voucher_types vt
                       ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
                     JOIN voucher_lines vl
                       ON vl.company_id = v.company_id AND vl.voucher_id = v.id
                     JOIN ledgers l
                       ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                     WHERE v.company_id = ?
                       AND v.status IN ('posted', 'reversed')
                       AND v.date >= ? AND v.date <= ?",
                );
                if filter.ledger_id.is_some() {
                    sql.push_str(" AND vl.ledger_id = ?");
                }
                sql.push_str(
                    " ORDER BY v.date ASC, v.sequence_number IS NULL ASC,
                              v.sequence_number ASC, v.id ASC,
                              vl.line_number ASC, vl.id ASC",
                );

                let mut query = sqlx::query_as::<_, ReportMovement>(&sql)
                    .bind(&company_id)
                    .bind(&filter.period.date_from)
                    .bind(&filter.period.date_to);
                if let Some(ledger_id) = filter.ledger_id {
                    query = query.bind(ledger_id);
                }
                query
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)
            })
        })
        .await
    }

    pub async fn list_opening_balances(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year_begins: &str,
        ledger_id: Option<&str>,
    ) -> Result<Vec<ReportOpeningBalance>, OpenTallyError> {
        let company_id = company_id.to_string();
        let financial_year_begins = financial_year_begins.to_string();
        let ledger_id = ledger_id.map(str::to_string);
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let mut sql = String::from(
                    "SELECT ledger_id, balance_type, amount
                     FROM opening_balances
                     WHERE company_id = ? AND financial_year_begins = ?",
                );
                if ledger_id.is_some() {
                    sql.push_str(" AND ledger_id = ?");
                }
                sql.push_str(" ORDER BY ledger_id ASC");
                let mut query = sqlx::query_as::<_, ReportOpeningBalance>(&sql)
                    .bind(company_id)
                    .bind(financial_year_begins);
                if let Some(ledger_id) = ledger_id {
                    query = query.bind(ledger_id);
                }
                query
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)
            })
        })
        .await
    }
}
