use super::models::*;
use crate::errors::OpenTallyError;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rust_decimal::Decimal;
use std::str::FromStr;

pub fn parse_tally_xml(xml: &str) -> Result<TallyXmlDocument, OpenTallyError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut doc = TallyXmlDocument::default();

    #[derive(Debug, PartialEq, Eq)]
    enum State {
        Root,
        InGroup,
        InLedger,
        InUnit,
        InGodown,
        InStockGroup,
        InStockItem,
        InVoucher,
        InVoucherLine,
    }

    let mut state = State::Root;
    let mut current_tag = String::new();

    // Context objects
    let mut cur_group = TallyXmlGroup {
        name: String::new(),
        parent: None,
        is_calculatable: false,
    };
    let mut cur_ledger = TallyXmlLedger {
        name: String::new(),
        parent: String::new(),
        opening_balance: Decimal::ZERO,
        is_deemed_positive: true,
    };
    let mut cur_unit = TallyXmlUnit {
        symbol: String::new(),
        formal_name: String::new(),
        decimal_places: 0,
    };
    let mut cur_godown = TallyXmlGodown {
        name: String::new(),
        parent: None,
    };
    let mut cur_stock_group = TallyXmlStockGroup {
        name: String::new(),
        parent: None,
    };
    let mut cur_stock_item = TallyXmlStockItem {
        name: String::new(),
        parent: None,
        base_units: String::new(),
        opening_balance_qty: Decimal::ZERO,
        opening_balance_rate: Decimal::ZERO,
        opening_balance_val: Decimal::ZERO,
    };
    let mut cur_voucher = TallyXmlVoucher {
        voucher_type: String::new(),
        voucher_number: String::new(),
        date: String::new(),
        narration: None,
        lines: Vec::new(),
    };
    let mut cur_line = TallyXmlVoucherLine {
        ledger_name: String::new(),
        entry_type: "Dr".to_string(),
        amount: Decimal::ZERO,
        narration: None,
    };
    let mut line_deemed_positive: Option<bool> = None;
    let mut raw_line_amount: Decimal = Decimal::ZERO;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.local_name();
                let tag_str = name.as_ref();
                let tag_upper = tag_str.to_ascii_uppercase();
                current_tag = tag_upper.clone();

                match tag_upper.as_str() {
                    "GROUP" => {
                        state = State::InGroup;
                        cur_group = TallyXmlGroup {
                            name: get_attr(e, "NAME").unwrap_or_default(),
                            parent: None,
                            is_calculatable: false,
                        };
                    }
                    "LEDGER" => {
                        state = State::InLedger;
                        cur_ledger = TallyXmlLedger {
                            name: get_attr(e, "NAME").unwrap_or_default(),
                            parent: String::new(),
                            opening_balance: Decimal::ZERO,
                            is_deemed_positive: true,
                        };
                    }
                    "UNIT" => {
                        state = State::InUnit;
                        let name_attr = get_attr(e, "NAME")
                            .or_else(|| get_attr(e, "IDENTIFIER"))
                            .unwrap_or_default();
                        cur_unit = TallyXmlUnit {
                            symbol: name_attr.clone(),
                            formal_name: name_attr,
                            decimal_places: 0,
                        };
                    }
                    "GODOWN" => {
                        state = State::InGodown;
                        cur_godown = TallyXmlGodown {
                            name: get_attr(e, "NAME").unwrap_or_default(),
                            parent: None,
                        };
                    }
                    "STOCKGROUP" => {
                        state = State::InStockGroup;
                        cur_stock_group = TallyXmlStockGroup {
                            name: get_attr(e, "NAME").unwrap_or_default(),
                            parent: None,
                        };
                    }
                    "STOCKITEM" => {
                        state = State::InStockItem;
                        cur_stock_item = TallyXmlStockItem {
                            name: get_attr(e, "NAME").unwrap_or_default(),
                            parent: None,
                            base_units: String::new(),
                            opening_balance_qty: Decimal::ZERO,
                            opening_balance_rate: Decimal::ZERO,
                            opening_balance_val: Decimal::ZERO,
                        };
                    }
                    "VOUCHER" => {
                        state = State::InVoucher;
                        cur_voucher = TallyXmlVoucher {
                            voucher_type: get_attr(e, "VCHTYPE").unwrap_or_default(),
                            voucher_number: String::new(),
                            date: String::new(),
                            narration: None,
                            lines: Vec::new(),
                        };
                    }
                    "ALLLEDGERENTRIES.LIST" | "LEDGERENTRIES.LIST" => {
                        state = State::InVoucherLine;
                        cur_line = TallyXmlVoucherLine {
                            ledger_name: String::new(),
                            entry_type: "Dr".to_string(),
                            amount: Decimal::ZERO,
                            narration: None,
                        };
                        line_deemed_positive = None;
                        raw_line_amount = Decimal::ZERO;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let raw_text = e.as_ref();
                let text = quick_xml::escape::unescape(raw_text)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if text.is_empty() {
                    continue;
                }

                match state {
                    State::InGroup => match current_tag.as_str() {
                        "NAME" if cur_group.name.is_empty() => cur_group.name = text,
                        "PARENT" => cur_group.parent = Some(text),
                        "BASICGROUPISCALCULATABLE" => {
                            cur_group.is_calculatable = text.eq_ignore_ascii_case("Yes");
                        }
                        _ => {}
                    },
                    State::InLedger => match current_tag.as_str() {
                        "NAME" if cur_ledger.name.is_empty() => cur_ledger.name = text,
                        "PARENT" => cur_ledger.parent = text,
                        "OPENINGBALANCE" => {
                            if let Ok(d) = Decimal::from_str(&text) {
                                cur_ledger.opening_balance = d.abs();
                                if d < Decimal::ZERO {
                                    cur_ledger.is_deemed_positive = true;
                                }
                            }
                        }
                        "ISDEEMEDPOSITIVE" => {
                            cur_ledger.is_deemed_positive = text.eq_ignore_ascii_case("Yes");
                        }
                        _ => {}
                    },
                    State::InUnit => match current_tag.as_str() {
                        "NAME" | "ISSYMBOL" if cur_unit.symbol.is_empty() => {
                            cur_unit.symbol = text.clone();
                            if cur_unit.formal_name.is_empty() {
                                cur_unit.formal_name = text;
                            }
                        }
                        "FORMALNAME" => cur_unit.formal_name = text,
                        "DECIMALPLACES" => cur_unit.decimal_places = text.parse().unwrap_or(0),
                        _ => {}
                    },
                    State::InGodown => match current_tag.as_str() {
                        "NAME" if cur_godown.name.is_empty() => cur_godown.name = text,
                        "PARENT" => cur_godown.parent = Some(text),
                        _ => {}
                    },
                    State::InStockGroup => match current_tag.as_str() {
                        "NAME" if cur_stock_group.name.is_empty() => cur_stock_group.name = text,
                        "PARENT" => cur_stock_group.parent = Some(text),
                        _ => {}
                    },
                    State::InStockItem => match current_tag.as_str() {
                        "NAME" if cur_stock_item.name.is_empty() => cur_stock_item.name = text,
                        "PARENT" => cur_stock_item.parent = Some(text),
                        "BASEUNITS" => cur_stock_item.base_units = text,
                        "OPENINGBALANCE" => {
                            // Can be "10 Pcs" or "10"
                            let num_part = text.split_whitespace().next().unwrap_or("0");
                            if let Ok(d) = Decimal::from_str(num_part) {
                                cur_stock_item.opening_balance_qty = d.abs();
                            }
                        }
                        "OPENINGRATE" => {
                            let clean_rate = text.split('/').next().unwrap_or("0");
                            let num_part = clean_rate.split_whitespace().next().unwrap_or("0");
                            if let Ok(d) = Decimal::from_str(num_part) {
                                cur_stock_item.opening_balance_rate = d.abs();
                            }
                        }
                        "OPENINGVALUE" => {
                            if let Ok(d) = Decimal::from_str(&text) {
                                cur_stock_item.opening_balance_val = d.abs();
                            }
                        }
                        _ => {}
                    },
                    State::InVoucher => match current_tag.as_str() {
                        "DATE" => cur_voucher.date = normalize_tally_date(&text),
                        "VOUCHERTYPENAME" if cur_voucher.voucher_type.is_empty() => {
                            cur_voucher.voucher_type = text;
                        }
                        "VOUCHERNUMBER" => cur_voucher.voucher_number = text,
                        "NARRATION" => cur_voucher.narration = Some(text),
                        _ => {}
                    },
                    State::InVoucherLine => match current_tag.as_str() {
                        "LEDGERNAME" => cur_line.ledger_name = text,
                        "ISDEEMEDPOSITIVE" => {
                            line_deemed_positive = Some(text.eq_ignore_ascii_case("Yes"));
                        }
                        "AMOUNT" => {
                            if let Ok(d) = Decimal::from_str(&text) {
                                raw_line_amount = d;
                            }
                        }
                        "NARRATION" => cur_line.narration = Some(text),
                        _ => {}
                    },
                    State::Root => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                let tag_str = name.as_ref();
                let tag_upper = tag_str.to_ascii_uppercase();

                match tag_upper.as_str() {
                    "GROUP" => {
                        if !cur_group.name.is_empty() {
                            doc.groups.push(cur_group.clone());
                        }
                        state = State::Root;
                    }
                    "LEDGER" => {
                        if !cur_ledger.name.is_empty() {
                            doc.ledgers.push(cur_ledger.clone());
                        }
                        state = State::Root;
                    }
                    "UNIT" => {
                        if !cur_unit.symbol.is_empty() {
                            doc.units.push(cur_unit.clone());
                        }
                        state = State::Root;
                    }
                    "GODOWN" => {
                        if !cur_godown.name.is_empty() {
                            doc.godowns.push(cur_godown.clone());
                        }
                        state = State::Root;
                    }
                    "STOCKGROUP" => {
                        if !cur_stock_group.name.is_empty() {
                            doc.stock_groups.push(cur_stock_group.clone());
                        }
                        state = State::Root;
                    }
                    "STOCKITEM" => {
                        if !cur_stock_item.name.is_empty() {
                            doc.stock_items.push(cur_stock_item.clone());
                        }
                        state = State::Root;
                    }
                    "ALLLEDGERENTRIES.LIST" | "LEDGERENTRIES.LIST" => {
                        if !cur_line.ledger_name.is_empty() {
                            // Resolve entry_type and positive amount:
                            // In Tally XML:
                            // If ISDEEMEDPOSITIVE == "Yes", it's a Debit (Dr)
                            // If ISDEEMEDPOSITIVE == "No", it's a Credit (Cr)
                            // If ISDEEMEDPOSITIVE is missing, negative amount indicates Dr, positive Cr
                            let is_dr = match line_deemed_positive {
                                Some(dp) => dp,
                                None => raw_line_amount < Decimal::ZERO,
                            };
                            cur_line.entry_type = if is_dr {
                                "Dr".to_string()
                            } else {
                                "Cr".to_string()
                            };
                            cur_line.amount = raw_line_amount.abs().round_dp(2);
                            cur_voucher.lines.push(cur_line.clone());
                        }
                        state = State::InVoucher;
                    }
                    "VOUCHER" => {
                        if !cur_voucher.voucher_number.is_empty() || !cur_voucher.lines.is_empty() {
                            doc.vouchers.push(cur_voucher.clone());
                        }
                        state = State::Root;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OpenTallyError::Validation(format!(
                    "XML parser syntax error at position {}: {:?}",
                    reader.buffer_position(),
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(doc)
}

fn get_attr(e: &quick_xml::events::BytesStart, attr_name: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        let name = a.key.local_name();
        let key = name.as_ref();
        if key.eq_ignore_ascii_case(attr_name) {
            let val_raw = a.value.as_ref();
            let val = quick_xml::escape::unescape(val_raw).unwrap_or_default();
            return Some(val.to_string());
        }
    }
    None
}

/// Normalizes dates into standard ISO `YYYY-MM-DD`.
/// Handles `YYYYMMDD`, `YYYY-MM-DD`, and `DD-MM-YYYY`.
pub fn normalize_tally_date(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() == 8 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &trimmed[0..4], &trimmed[4..6], &trimmed[6..8])
    } else if trimmed.len() == 10 && trimmed.contains('-') {
        let parts: Vec<&str> = trimmed.split('-').collect();
        if parts.len() == 3 {
            if parts[0].len() == 4 {
                trimmed.to_string()
            } else if parts[2].len() == 4 {
                format!("{}-{:0>2}-{:0>2}", parts[2], parts[1], parts[0])
            } else {
                trimmed.to_string()
            }
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    }
}
