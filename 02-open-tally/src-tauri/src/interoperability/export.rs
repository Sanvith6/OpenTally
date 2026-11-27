use super::security::escape_xml;
use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::fmt::Write as FmtWrite;
use std::str::FromStr;

/// Exports company masters and posted transactions in canonical Tally XML format.
/// Executes strictly inside `ReadOnlyConnectionGuard` to guarantee zero state mutation.
pub async fn export_company_tally_xml(
    pool: &Pool<Sqlite>,
    company_id: &str,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> Result<String, OpenTallyError> {
    let company_id_owned = company_id.to_string();
    let from_date_owned = from_date.map(|s| s.to_string());
    let to_date_owned = to_date.map(|s| s.to_string());

    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            // Step 1: Verify company exists
            let company_exists: Option<String> = sqlx::query_scalar(
                "SELECT name FROM companies WHERE id = ?",
            )
            .bind(&company_id_owned)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if company_exists.is_none() {
                return Err(OpenTallyError::NotFound(format!(
                    "Company '{}' not found",
                    company_id_owned
                )));
            }

            let mut xml = String::with_capacity(32768);
            xml.push_str("<ENVELOPE>\n  <HEADER>\n    <TALLYREQUEST>Import Data</TALLYREQUEST>\n  </HEADER>\n  <BODY>\n    <IMPORTDATA>\n      <REQUESTDATA>\n");

            // Step 2: Custom Account Groups
            #[derive(sqlx::FromRow)]
            struct GroupRow {
                name: String,
                parent_name: Option<String>,
            }

            let groups = sqlx::query_as::<_, GroupRow>(
                "SELECT g.name, p.name as parent_name
                 FROM account_groups g
                 LEFT JOIN account_groups p ON p.company_id = g.company_id AND p.id = g.parent_id
                 WHERE g.company_id = ? AND g.is_predefined = 0
                 ORDER BY g.created_at ASC",
            )
            .bind(&company_id_owned)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            for g in groups {
                let parent_val = g.parent_name.unwrap_or_else(|| "Primary".to_string());
                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <GROUP NAME=\"{}\" RESERVEDNAME=\"\">\n            <PARENT>{}</PARENT>\n            <BASICGROUPISCALCULATABLE>No</BASICGROUPISCALCULATABLE>\n          </GROUP>\n        </TALLYMESSAGE>",
                    escape_xml(&g.name),
                    escape_xml(&parent_val)
                );
            }

            // Step 3: Ledgers
            #[derive(sqlx::FromRow)]
            struct LedgerRow {
                name: String,
                parent_name: String,
                opening_balance: String,
                opening_balance_type: String,
            }

            let ledgers = sqlx::query_as::<_, LedgerRow>(
                "SELECT l.name, g.name as parent_name,
                        COALESCE(ob.amount, '0.00') as opening_balance,
                        COALESCE(ob.balance_type, 'Dr') as opening_balance_type
                 FROM ledgers l
                 JOIN account_groups g ON g.company_id = l.company_id AND g.id = l.group_id
                 LEFT JOIN opening_balances ob ON ob.company_id = l.company_id AND ob.ledger_id = l.id
                 WHERE l.company_id = ?
                 ORDER BY l.name ASC",
            )
            .bind(&company_id_owned)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            for l in ledgers {
                let op_bal = Decimal::from_str(&l.opening_balance).unwrap_or(Decimal::ZERO);
                let is_dr = l.opening_balance_type == "Dr";
                let deemed_positive = if is_dr { "Yes" } else { "No" };
                let amount_str = if is_dr && op_bal > Decimal::ZERO {
                    format!("-{:.2}", op_bal)
                } else {
                    format!("{:.2}", op_bal)
                };

                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <LEDGER NAME=\"{}\" RESERVEDNAME=\"\">\n            <PARENT>{}</PARENT>\n            <OPENINGBALANCE>{}</OPENINGBALANCE>\n            <ISDEEMEDPOSITIVE>{}</ISDEEMEDPOSITIVE>\n          </LEDGER>\n        </TALLYMESSAGE>",
                    escape_xml(&l.name),
                    escape_xml(&l.parent_name),
                    amount_str,
                    deemed_positive
                );
            }

            // Step 4: Units
            #[derive(sqlx::FromRow)]
            struct UnitRow {
                symbol: String,
                formal_name: String,
                decimal_precision: i64,
            }

            let units = sqlx::query_as::<_, UnitRow>(
                "SELECT symbol, formal_name, decimal_precision
                 FROM units
                 WHERE company_id = ?
                 ORDER BY symbol ASC",
            )
            .bind(&company_id_owned)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for u in units {
                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <UNIT NAME=\"{}\" IDENTIFIER=\"{}\">\n            <ISSYMBOL>{}</ISSYMBOL>\n            <FORMALNAME>{}</FORMALNAME>\n            <DECIMALPLACES>{}</DECIMALPLACES>\n          </UNIT>\n        </TALLYMESSAGE>",
                    escape_xml(&u.symbol),
                    escape_xml(&u.symbol),
                    escape_xml(&u.symbol),
                    escape_xml(&u.formal_name),
                    u.decimal_precision
                );
            }

            // Step 5: Godowns
            #[derive(sqlx::FromRow)]
            struct GodownRow {
                name: String,
                parent_name: Option<String>,
            }

            let godowns = sqlx::query_as::<_, GodownRow>(
                "SELECT g.name, p.name as parent_name
                 FROM godowns g
                 LEFT JOIN godowns p ON p.company_id = g.company_id AND p.id = g.parent_id
                 WHERE g.company_id = ?
                 ORDER BY g.name ASC",
            )
            .bind(&company_id_owned)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for gd in godowns {
                let parent_tag = match gd.parent_name {
                    Some(ref p) => format!("<PARENT>{}</PARENT>", escape_xml(p)),
                    None => "<PARENT/>".to_string(),
                };
                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <GODOWN NAME=\"{}\" RESERVEDNAME=\"\">\n            {}\n          </GODOWN>\n        </TALLYMESSAGE>",
                    escape_xml(&gd.name),
                    parent_tag
                );
            }

            // Step 6: Stock Groups
            #[derive(sqlx::FromRow)]
            struct StockGroupRow {
                name: String,
                parent_name: Option<String>,
            }

            let stock_groups = sqlx::query_as::<_, StockGroupRow>(
                "SELECT g.name, p.name as parent_name
                 FROM stock_groups g
                 LEFT JOIN stock_groups p ON p.company_id = g.company_id AND p.id = g.parent_id
                 WHERE g.company_id = ?
                 ORDER BY g.name ASC",
            )
            .bind(&company_id_owned)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for sg in stock_groups {
                let parent_tag = match sg.parent_name {
                    Some(ref p) => format!("<PARENT>{}</PARENT>", escape_xml(p)),
                    None => "<PARENT/>".to_string(),
                };
                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <STOCKGROUP NAME=\"{}\" RESERVEDNAME=\"\">\n            {}\n          </STOCKGROUP>\n        </TALLYMESSAGE>",
                    escape_xml(&sg.name),
                    parent_tag
                );
            }

            // Step 7: Stock Items
            #[derive(sqlx::FromRow)]
            struct StockItemRow {
                name: String,
                group_name: String,
                unit_symbol: String,
                opening_quantity: String,
                opening_rate: String,
                opening_value: String,
            }

            let stock_items = sqlx::query_as::<_, StockItemRow>(
                "SELECT si.name, sg.name as group_name, u.symbol as unit_symbol,
                        si.opening_quantity, si.opening_rate, si.opening_value
                 FROM stock_items si
                 JOIN stock_groups sg ON sg.company_id = si.company_id AND sg.id = si.stock_group_id
                 JOIN units u ON u.company_id = si.company_id AND u.id = si.base_unit_id
                 WHERE si.company_id = ?
                 ORDER BY si.name ASC",
            )
            .bind(&company_id_owned)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for itm in stock_items {
                let qty = Decimal::from_str(&itm.opening_quantity).unwrap_or(Decimal::ZERO);
                let rate = Decimal::from_str(&itm.opening_rate).unwrap_or(Decimal::ZERO);
                let val = Decimal::from_str(&itm.opening_value).unwrap_or(Decimal::ZERO);
                let val_str = if val > Decimal::ZERO {
                    format!("-{:.2}", val)
                } else {
                    format!("{:.2}", val)
                };

                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <STOCKITEM NAME=\"{}\" RESERVEDNAME=\"\">\n            <PARENT>{}</PARENT>\n            <BASEUNITS>{}</BASEUNITS>\n            <OPENINGBALANCE>{} {}</OPENINGBALANCE>\n            <OPENINGRATE>{:.2}</OPENINGRATE>\n            <OPENINGVALUE>{}</OPENINGVALUE>\n          </STOCKITEM>\n        </TALLYMESSAGE>",
                    escape_xml(&itm.name),
                    escape_xml(&itm.group_name),
                    escape_xml(&itm.unit_symbol),
                    qty,
                    escape_xml(&itm.unit_symbol),
                    rate,
                    val_str
                );
            }

            // Step 8: Posted Vouchers
            #[derive(sqlx::FromRow)]
            struct VoucherRow {
                id: String,
                voucher_type_name: String,
                voucher_number: String,
                date: String,
                narration: Option<String>,
            }

            let mut vch_query = "SELECT v.id, vt.name as voucher_type_name, v.voucher_number, v.date, v.narration
                                 FROM vouchers v
                                 JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
                                 WHERE v.company_id = ? AND v.status = 'posted'".to_string();

            if from_date_owned.is_some() {
                vch_query.push_str(" AND v.date >= ?");
            }
            if to_date_owned.is_some() {
                vch_query.push_str(" AND v.date <= ?");
            }
            vch_query.push_str(" ORDER BY v.date ASC, v.voucher_number ASC");

            let mut q = sqlx::query_as::<_, VoucherRow>(&vch_query).bind(&company_id_owned);
            if let Some(ref fd) = from_date_owned {
                q = q.bind(fd);
            }
            if let Some(ref td) = to_date_owned {
                q = q.bind(td);
            }

            let vouchers = q.fetch_all(&mut *conn).await.map_err(OpenTallyError::Database)?;

            #[derive(sqlx::FromRow)]
            struct LineRow {
                id: String,
                ledger_name: String,
                entry_type: String,
                amount: String,
                narration: Option<String>,
            }

            for v in vouchers {
                let date_compact = v.date.replace('-', "");
                let vch_narr = v.narration.as_deref().unwrap_or("");

                let _ = writeln!(
                    xml,
                    "        <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">\n          <VOUCHER VCHTYPE=\"{}\" ACTION=\"Create\">\n            <DATE>{}</DATE>\n            <VOUCHERTYPENAME>{}</VOUCHERTYPENAME>\n            <VOUCHERNUMBER>{}</VOUCHERNUMBER>\n            <NARRATION>{}</NARRATION>",
                    escape_xml(&v.voucher_type_name),
                    date_compact,
                    escape_xml(&v.voucher_type_name),
                    escape_xml(&v.voucher_number),
                    escape_xml(vch_narr)
                );

                let lines = sqlx::query_as::<_, LineRow>(
                    "SELECT vl.id, l.name as ledger_name, vl.entry_type, vl.amount, vl.narration
                     FROM voucher_lines vl
                     JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                     WHERE vl.company_id = ? AND vl.voucher_id = ?
                     ORDER BY vl.line_number ASC",
                )
                .bind(&company_id_owned)
                .bind(&v.id)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                for l in lines {
                    let amt = Decimal::from_str(&l.amount).unwrap_or(Decimal::ZERO);
                    let is_dr = l.entry_type == "Dr";
                    let deemed_positive = if is_dr { "Yes" } else { "No" };
                    let amt_str = if is_dr {
                        format!("-{:.2}", amt)
                    } else {
                        format!("{:.2}", amt)
                    };

                    let line_narr = l.narration.as_deref().unwrap_or("");

                    let _ = writeln!(
                        xml,
                        "            <ALLLEDGERENTRIES.LIST>\n              <LEDGERNAME>{}</LEDGERNAME>\n              <ISDEEMEDPOSITIVE>{}</ISDEEMEDPOSITIVE>\n              <AMOUNT>{}</AMOUNT>\n              <NARRATION>{}</NARRATION>",
                        escape_xml(&l.ledger_name),
                        deemed_positive,
                        amt_str,
                        escape_xml(line_narr)
                    );

                    // Emit nested BILLALLOCATIONS.LIST if present
                    let bills: Vec<(String, String, String)> = sqlx::query_as(
                        "SELECT bill_name, allocation_type, amount FROM bill_allocations WHERE company_id = ? AND voucher_id = ? AND voucher_line_id = ?",
                    )
                    .bind(&company_id_owned)
                    .bind(&v.id)
                    .bind(&l.id)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();
                    for (bname, btype, bamt) in bills {
                        let _ = writeln!(
                            xml,
                            "              <BILLALLOCATIONS.LIST>\n                <NAME>{}</NAME>\n                <BILLTYPE>{}</BILLTYPE>\n                <AMOUNT>{}</AMOUNT>\n              </BILLALLOCATIONS.LIST>",
                            escape_xml(&bname),
                            escape_xml(&btype),
                            bamt
                        );
                    }

                    // Emit nested BANKALLOCATIONS.LIST if present
                    type BankExportRow = (
                        String,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        String,
                    );
                    let banks: Vec<BankExportRow> = sqlx::query_as(
                        "SELECT transaction_type, instrument_number, instrument_date, bank_date, favouring_name, amount FROM bank_allocations WHERE company_id = ? AND voucher_id = ? AND voucher_line_id = ?",
                    )
                    .bind(&company_id_owned)
                    .bind(&v.id)
                    .bind(&l.id)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();
                    for (ttype, inum, idate, bdate, fav, bamt) in banks {
                        let _ = writeln!(
                            xml,
                            "              <BANKALLOCATIONS.LIST>\n                <TRANSACTIONTYPE>{}</TRANSACTIONTYPE>\n                <INSTRUMENTNUMBER>{}</INSTRUMENTNUMBER>\n                <INSTRUMENTDATE>{}</INSTRUMENTDATE>\n                <BANKERSDATE>{}</BANKERSDATE>\n                <PAYMENTFAVOURING>{}</PAYMENTFAVOURING>\n                <AMOUNT>{}</AMOUNT>\n              </BANKALLOCATIONS.LIST>",
                            escape_xml(&ttype),
                            escape_xml(inum.as_deref().unwrap_or("")),
                            escape_xml(idate.as_deref().unwrap_or("")),
                            escape_xml(bdate.as_deref().unwrap_or("")),
                            escape_xml(fav.as_deref().unwrap_or("")),
                            bamt
                        );
                    }

                    // Emit nested CATEGORYALLOCATIONS.LIST -> COSTCENTREALLOCATIONS.LIST
                    let costs: Vec<(String, String, String)> = sqlx::query_as(
                        "SELECT COALESCE(cat.name, 'Primary Cost Category'), cc.name, vca.amount
                         FROM voucher_cost_allocations vca
                         JOIN cost_centres cc ON cc.id = vca.cost_centre_id
                         LEFT JOIN cost_categories cat ON cat.id = cc.category_id
                         WHERE vca.company_id = ? AND vca.voucher_id = ? AND vca.voucher_line_id = ?",
                    )
                    .bind(&company_id_owned)
                    .bind(&v.id)
                    .bind(&l.id)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();
                    for (cat_name, cc_name, camt) in costs {
                        let _ = writeln!(
                            xml,
                            "              <CATEGORYALLOCATIONS.LIST>\n                <CATEGORY>{}</CATEGORY>\n                <COSTCENTREALLOCATIONS.LIST>\n                  <NAME>{}</NAME>\n                  <AMOUNT>{}</AMOUNT>\n                </COSTCENTREALLOCATIONS.LIST>\n              </CATEGORYALLOCATIONS.LIST>",
                            escape_xml(&cat_name),
                            escape_xml(&cc_name),
                            camt
                        );
                    }

                    xml.push_str("            </ALLLEDGERENTRIES.LIST>\n");
                }

                // Emit ALLINVENTORYENTRIES.LIST for linked stock_vouchers
                let inv_rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
                    "SELECT si.name, svl.quantity, svl.unit_cost, svl.line_value, g.name
                     FROM stock_vouchers sv
                     JOIN stock_voucher_lines svl ON svl.stock_voucher_id = sv.id
                     JOIN stock_items si ON si.id = svl.stock_item_id
                     LEFT JOIN godowns g ON g.id = svl.godown_id
                     WHERE sv.company_id = ? AND sv.accounting_voucher_id = ?",
                )
                .bind(&company_id_owned)
                .bind(&v.id)
                .fetch_all(&mut *conn)
                .await
                .unwrap_or_default();
                for (iname, iqty, irate, ival, gname) in inv_rows {
                    let _ = writeln!(
                        xml,
                        "            <ALLINVENTORYENTRIES.LIST>\n              <STOCKITEMNAME>{}</STOCKITEMNAME>\n              <ACTUALQTY>{}</ACTUALQTY>\n              <BILLEDQTY>{}</BILLEDQTY>\n              <RATE>{}</RATE>\n              <AMOUNT>{}</AMOUNT>\n              <BATCHALLOCATIONS.LIST>\n                <GODOWNNAME>{}</GODOWNNAME>\n                <AMOUNT>{}</AMOUNT>\n              </BATCHALLOCATIONS.LIST>\n            </ALLINVENTORYENTRIES.LIST>",
                        escape_xml(&iname),
                        iqty,
                        iqty,
                        irate,
                        ival,
                        escape_xml(gname.as_deref().unwrap_or("Main Location")),
                        ival
                    );
                }

                xml.push_str("          </VOUCHER>\n        </TALLYMESSAGE>\n");
            }

            xml.push_str("      </REQUESTDATA>\n    </IMPORTDATA>\n  </BODY>\n</ENVELOPE>\n");
            Ok(xml)
        })
    })
    .await
}
