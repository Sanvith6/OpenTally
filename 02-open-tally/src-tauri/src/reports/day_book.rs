use crate::errors::OpenTallyError;
use crate::reports::calculation::parse_report_amount;
use crate::reports::model::{
    DayBookEntry, DayBookLine, DayBookRequest, ReportPage, ReportPageRequest,
};
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::{Pool, Sqlite};

#[derive(Debug, Deserialize, sqlx::FromRow)]
struct DayBookHeader {
    voucher_id: String,
    date: String,
    voucher_number: String,
    voucher_type: String,
    narration: Option<String>,
    total_amount: Option<String>,
}

#[derive(Debug, Deserialize, sqlx::FromRow)]
struct DayBookDetail {
    voucher_id: String,
    ledger_id: String,
    ledger_name: String,
    entry_type: String,
    amount: String,
    narration: Option<String>,
}

pub struct DayBookService;

impl DayBookService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: DayBookRequest,
    ) -> Result<ReportPage<DayBookEntry>, OpenTallyError> {
        request
            .page
            .validate()
            .map_err(OpenTallyError::Validation)?;
        let offset = (request.page.page - 1)
            .checked_mul(request.page.page_size)
            .ok_or_else(|| {
                OpenTallyError::Validation("Report pagination offset overflow".into())
            })?;
        let company_id = company_id.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let (where_sql, mut params) = Self::filter_sql(&request);
                params[0] = company_id.clone();
                let count_sql = format!("SELECT COUNT(*) FROM vouchers v WHERE {where_sql}");
                let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
                for param in &params {
                    count_query = count_query.bind(param);
                }
                let total = usize::try_from(
                    count_query
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?,
                )
                .map_err(|_| OpenTallyError::Validation("Report count overflow".into()))?;

                let page_sql = format!(
                    "SELECT v.id AS voucher_id, v.date, v.voucher_number,
                            vt.name AS voucher_type, v.narration, v.total_amount
                     FROM vouchers v
                     JOIN voucher_types vt
                       ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
                     WHERE {where_sql}
                     ORDER BY v.date ASC, v.sequence_number IS NULL ASC,
                              v.sequence_number ASC, v.id ASC
                     LIMIT ? OFFSET ?"
                );
                let mut page_query = sqlx::query_as::<_, DayBookHeader>(&page_sql);
                for param in &params {
                    page_query = page_query.bind(param);
                }
                let headers = page_query
                    .bind(request.page.page_size as i64)
                    .bind(offset as i64)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                let details = if headers.is_empty() {
                    Vec::new()
                } else {
                    let placeholders = std::iter::repeat_n("?", headers.len())
                        .collect::<Vec<_>>()
                        .join(",");
                    let detail_sql = format!(
                        "SELECT vl.voucher_id, vl.ledger_id, l.name AS ledger_name,
                                vl.entry_type, vl.amount, vl.narration
                         FROM voucher_lines vl
                         JOIN ledgers l
                           ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                         WHERE vl.company_id = ? AND vl.voucher_id IN ({placeholders})
                         ORDER BY vl.voucher_id ASC, vl.line_number ASC, vl.id ASC"
                    );
                    let mut detail_query =
                        sqlx::query_as::<_, DayBookDetail>(&detail_sql).bind(&company_id);
                    for header in &headers {
                        detail_query = detail_query.bind(&header.voucher_id);
                    }
                    detail_query
                        .fetch_all(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?
                };

                let mut entries = Vec::with_capacity(headers.len());
                for header in headers {
                    let mut debit = Decimal::ZERO;
                    let mut credit = Decimal::ZERO;
                    let lines = details
                        .iter()
                        .filter(|detail| detail.voucher_id == header.voucher_id)
                        .map(|detail| {
                            let amount = parse_report_amount(&detail.amount)?;
                            match detail.entry_type.as_str() {
                                "Dr" => debit += amount,
                                "Cr" => credit += amount,
                                other => {
                                    return Err(OpenTallyError::Validation(format!(
                                        "Invalid entry type '{}' in voucher '{}'",
                                        other, header.voucher_number
                                    )))
                                }
                            }
                            Ok(DayBookLine {
                                ledger_id: detail.ledger_id.clone(),
                                ledger_name: detail.ledger_name.clone(),
                                debit: if detail.entry_type == "Dr" {
                                    detail.amount.clone()
                                } else {
                                    "0".to_string()
                                },
                                credit: if detail.entry_type == "Cr" {
                                    detail.amount.clone()
                                } else {
                                    "0".to_string()
                                },
                                narration: detail.narration.clone(),
                            })
                        })
                        .collect::<Result<Vec<_>, OpenTallyError>>()?;
                    let voucher_total = header.total_amount.unwrap_or_else(|| debit.to_string());
                    entries.push(DayBookEntry {
                        voucher_id: header.voucher_id,
                        date: header.date,
                        voucher_number: header.voucher_number,
                        voucher_type: header.voucher_type,
                        narration: header.narration,
                        debit_total: debit.to_string(),
                        credit_total: credit.to_string(),
                        voucher_total,
                        lines,
                    });
                }

                Ok(ReportPage {
                    items: entries,
                    total,
                    page: request.page.page,
                    page_size: request.page.page_size,
                    total_pages: total.div_ceil(request.page.page_size),
                })
            })
        })
        .await
    }

    fn filter_sql(request: &DayBookRequest) -> (String, Vec<String>) {
        let mut predicates = vec![
            "v.company_id = ?".to_string(),
            "v.status IN ('posted', 'reversed')".to_string(),
            "v.date >= ?".to_string(),
            "v.date <= ?".to_string(),
        ];
        let mut params = vec![
            String::new(),
            request.filter.period.date_from.clone(),
            request.filter.period.date_to.clone(),
        ];
        if let Some(voucher_type_id) = &request.voucher_type_id {
            predicates.push("v.voucher_type_id = ?".to_string());
            params.push(voucher_type_id.clone());
        }
        if let Some(query) = &request.voucher_number_query {
            predicates.push("v.voucher_number LIKE '%' || ? || '%' COLLATE NOCASE".to_string());
            params.push(query.clone());
        }
        if let Some(ledger_id) = &request.filter.ledger_id {
            predicates.push(
                "EXISTS (SELECT 1 FROM voucher_lines lf WHERE lf.company_id = v.company_id
                         AND lf.voucher_id = v.id AND lf.ledger_id = ?)"
                    .to_string(),
            );
            params.push(ledger_id.clone());
        }
        (predicates.join(" AND "), params)
    }

    pub fn default_request(date_from: &str, date_to: &str) -> Result<DayBookRequest, String> {
        Ok(DayBookRequest {
            filter: crate::reports::ReportFilter {
                period: crate::reports::ReportDateRange::new(date_from, date_to)?,
                ledger_id: None,
            },
            voucher_type_id: None,
            voucher_number_query: None,
            page: ReportPageRequest {
                page: 1,
                page_size: 25,
            },
        })
    }
}
