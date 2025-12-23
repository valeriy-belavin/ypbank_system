//! CAMT.053 (ISO 20022) format parser and serializer.
//!
//! CAMT.053 is an XML-based bank-to-customer account statement format
//! defined by the ISO 20022 standard.

use crate::error::{Error, Result};
use crate::types::{Balance, BalanceType, DebitCredit, Statement, Transaction};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::str::FromStr;

/// Represents a CAMT.053 statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Camt053Statement {
    /// The underlying statement data.
    pub statement: Statement,
}

impl Camt053Statement {
    /// Parse a CAMT.053 statement from any source implementing `Read`.
    ///
    /// # Arguments
    ///
    /// * `reader` - A mutable reference to a type implementing `Read`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ypbank_system::camt053_format::Camt053Statement;
    ///
    /// let mut file = File::open("statement.xml")?;
    /// let statement = Camt053Statement::from_read(&mut file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut xml_content = String::new();
        reader.read_to_string(&mut xml_content)?;

        let document: Document = serde_xml_rs::from_str(&xml_content)?;

        Self::from_document(document)
    }

    /// Write a CAMT.053 statement to any destination implementing `Write`.
    ///
    /// # Arguments
    ///
    /// * `writer` - A mutable reference to a type implementing `Write`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ypbank_system::camt053_format::Camt053Statement;
    /// use ypbank_system::types::Statement;
    ///
    /// let statement = Statement::new("123".into(), "ACC001".into(), "USD".into());
    /// let camt053 = Camt053Statement { statement };
    /// let mut file = File::create("output.xml")?;
    /// camt053.write_to(&mut file)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let document = self.to_document();
        let xml = serde_xml_rs::to_string(&document)
            .map_err(|e| Error::XmlError(e.to_string()))?;

        // Write XML declaration and formatted output
        writeln!(writer, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        write!(writer, "{}", xml)?;

        Ok(())
    }

    fn from_document(document: Document) -> Result<Self> {
        let stmt_data = &document.bk_to_cstmr_stmt.stmt;

        let statement_id = stmt_data.id.clone();
        let account_id = stmt_data.acct.id.iban.clone()
            .or_else(|| stmt_data.acct.id.othr.as_ref().map(|o| o.id.clone()))
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let currency = stmt_data.acct.ccy.clone();
        let sequence_number = stmt_data.elctrnic_seq_nb.as_ref().map(|n| n.to_string());

        let mut statement = Statement::new(statement_id, account_id, currency);
        statement.sequence_number = sequence_number;
        statement.account_holder = stmt_data.acct.nm.clone();

        // Parse creation date
        if let Some(ref cre_dt_tm) = stmt_data.cre_dt_tm {
            statement.creation_date = parse_camt_date(cre_dt_tm).ok();
        }

        // Parse date range
        if let Some(ref fr_to_dt) = stmt_data.fr_to_dt {
            if let Some(ref from) = fr_to_dt.fr_dt_tm {
                statement.from_date = parse_camt_date(from).ok();
            }
            if let Some(ref to) = fr_to_dt.to_dt_tm {
                statement.to_date = parse_camt_date(to).ok();
            }
        }

        // Parse balances
        for bal in &stmt_data.bal {
            let balance = Self::parse_balance(bal)?;
            match balance.balance_type {
                BalanceType::Opening => statement.opening_balance = Some(balance),
                BalanceType::Closing => statement.closing_balance = Some(balance),
                _ => {}
            }
        }

        // Parse transactions
        for entry in &stmt_data.ntry {
            let transaction = Self::parse_entry(entry, &statement.currency)?;
            statement.add_transaction(transaction);
        }

        Ok(Camt053Statement { statement })
    }

    fn parse_balance(bal: &BalanceXml) -> Result<Balance> {
        let balance_type = match bal.tp.cd_or_prtry.cd.as_str() {
            "OPBD" | "OPAV" => BalanceType::Opening,
            "CLBD" | "CLAV" => BalanceType::Closing,
            "PRCD" => BalanceType::Intermediate,
            _ => BalanceType::Intermediate,
        };

        let amount = Decimal::from_str(&bal.amt.value)
            .map_err(|_| Error::InvalidAmount(bal.amt.value.clone()))?;

        let debit_credit = bal.cdt_dbt_ind.parse::<DebitCredit>()
            .map_err(|_| Error::ParseError(format!("Invalid D/C indicator: {}", bal.cdt_dbt_ind)))?;

        let date = if let Some(ref dt) = bal.dt.dt {
            parse_date_only(dt)?
        } else if let Some(ref dt_tm) = bal.dt.dt_tm {
            parse_camt_date(dt_tm)?
        } else {
            return Err(Error::MissingField("balance date".to_string()));
        };

        let currency = bal.amt.ccy()
            .unwrap_or_else(|| "XXX".to_string());

        Ok(Balance {
            balance_type,
            amount,
            currency,
            debit_credit,
            date,
        })
    }

    fn parse_entry(entry: &EntryXml, default_currency: &str) -> Result<Transaction> {
        let reference = entry.ntry_ref.clone().unwrap_or_else(|| "UNKNOWN".to_string());

        let amount = Decimal::from_str(&entry.amt.value)
            .map_err(|_| Error::InvalidAmount(entry.amt.value.clone()))?;

        let debit_credit = entry.cdt_dbt_ind.parse::<DebitCredit>()
            .map_err(|_| Error::ParseError(format!("Invalid D/C indicator: {}", entry.cdt_dbt_ind)))?;

        let date = if let Some(ref dt) = entry.bookg_dt {
            if let Some(ref d) = dt.dt {
                parse_date_only(d)?
            } else if let Some(ref dt_tm) = dt.dt_tm {
                parse_camt_date(dt_tm)?
            } else {
                chrono::Utc::now().date_naive()
            }
        } else {
            chrono::Utc::now().date_naive()
        };

        let value_date = if let Some(ref dt) = entry.val_dt {
            if let Some(ref d) = dt.dt {
                Some(parse_date_only(d)?)
            } else if let Some(ref dt_tm) = dt.dt_tm {
                Some(parse_camt_date(dt_tm)?)
            } else {
                None
            }
        } else {
            None
        };

        let mut description = String::new();
        let mut counterparty_name = None;
        let mut counterparty_account = None;
        let mut bank_identifier = None;
        let mut additional_info = None;

        // Extract details from transaction details
        if let Some(ref ntry_dtls) = entry.ntry_dtls {
            if let Some(ref tx_dtls) = ntry_dtls.tx_dtls {
                // Remittance information
                if let Some(ref rmt_inf) = tx_dtls.rmt_inf {
                    if let Some(ref ustrd) = rmt_inf.ustrd {
                        description = ustrd.clone();
                    }
                }

                // Related parties
                if let Some(ref rltd_pties) = tx_dtls.rltd_pties {
                    if let Some(ref dbtr) = rltd_pties.dbtr {
                        counterparty_name = dbtr.nm.clone();
                    }
                    if let Some(ref cdtr) = rltd_pties.cdtr {
                        counterparty_name = cdtr.nm.clone();
                    }

                    if let Some(ref dbtr_acct) = rltd_pties.dbtr_acct {
                        counterparty_account = dbtr_acct.id.iban.clone()
                            .or_else(|| dbtr_acct.id.othr.as_ref().map(|o| o.id.clone()));
                    }
                    if let Some(ref cdtr_acct) = rltd_pties.cdtr_acct {
                        counterparty_account = cdtr_acct.id.iban.clone()
                            .or_else(|| cdtr_acct.id.othr.as_ref().map(|o| o.id.clone()));
                    }
                }

                // Related agents (banks)
                if let Some(ref rltd_agts) = tx_dtls.rltd_agts {
                    if let Some(ref dbtr_agt) = rltd_agts.dbtr_agt {
                        bank_identifier = dbtr_agt.fin_instn_id.bic.clone();
                    }
                    if let Some(ref cdtr_agt) = rltd_agts.cdtr_agt {
                        bank_identifier = cdtr_agt.fin_instn_id.bic.clone();
                    }
                }

                // Additional transaction info
                if let Some(ref addtl) = tx_dtls.addtl_tx_inf {
                    additional_info = Some(addtl.clone());
                }
            }
        }

        // Fallback to bank transaction code for description
        if description.is_empty() {
            if let Some(ref bk_tx_cd) = entry.bk_tx_cd {
                if let Some(ref prtry) = bk_tx_cd.prtry {
                    description = prtry.cd.clone();
                }
            }
        }

        Ok(Transaction {
            reference,
            date,
            value_date,
            amount,
            currency: entry.amt.ccy.clone().unwrap_or_else(|| default_currency.to_string()),
            debit_credit,
            account: None,
            counterparty_account,
            counterparty_name,
            bank_identifier,
            description,
            additional_info,
        })
    }

    fn to_document(&self) -> Document {
        let stmt = &self.statement;

        let mut balances = Vec::new();

        if let Some(ref opening) = stmt.opening_balance {
            balances.push(BalanceXml {
                tp: BalanceTypeXml {
                    cd_or_prtry: CodeOrProprietaryXml {
                        cd: "OPBD".to_string(),
                    },
                },
                amt: AmountXml {
                    value: opening.amount.to_string(),
                    ccy: Some(opening.currency.clone()),
                    ccy_alt: None,
                },
                cdt_dbt_ind: opening.debit_credit.to_iso_format().to_string(),
                dt: DateXml {
                    dt: Some(format_date_only(&opening.date)),
                    dt_tm: None,
                },
            });
        }

        if let Some(ref closing) = stmt.closing_balance {
            balances.push(BalanceXml {
                tp: BalanceTypeXml {
                    cd_or_prtry: CodeOrProprietaryXml {
                        cd: "CLBD".to_string(),
                    },
                },
                amt: AmountXml {
                    value: closing.amount.to_string(),
                    ccy: Some(closing.currency.clone()),
                    ccy_alt: None,
                },
                cdt_dbt_ind: closing.debit_credit.to_iso_format().to_string(),
                dt: DateXml {
                    dt: Some(format_date_only(&closing.date)),
                    dt_tm: None,
                },
            });
        }

        let entries: Vec<EntryXml> = stmt.transactions.iter().map(|tx| {
            EntryXml {
                ntry_ref: Some(tx.reference.clone()),
                amt: AmountXml {
                    value: tx.amount.to_string(),
                    ccy: Some(tx.currency.clone()),
                    ccy_alt: None,
                },
                cdt_dbt_ind: tx.debit_credit.to_iso_format().to_string(),
                sts: "BOOK".to_string(),
                bookg_dt: Some(DateXml {
                    dt: Some(format_date_only(&tx.date)),
                    dt_tm: None,
                }),
                val_dt: tx.value_date.as_ref().map(|vd| DateXml {
                    dt: Some(format_date_only(vd)),
                    dt_tm: None,
                }),
                acct_svcr_ref: None,
                bk_tx_cd: Some(BankTransactionCodeXml {
                    domn: None,
                    prtry: Some(ProprietaryCodeXml {
                        cd: tx.description.clone(),
                    }),
                }),
                ntry_dtls: Some(EntryDetailsXml {
                    tx_dtls: Some(TransactionDetailsXml {
                        refs: None,
                        amt_dtls: None,
                        rltd_pties: if tx.counterparty_name.is_some() || tx.counterparty_account.is_some() {
                            Some(RelatedPartiesXml {
                                dbtr: if tx.debit_credit == DebitCredit::Credit {
                                    tx.counterparty_name.as_ref().map(|name| PartyXml {
                                        nm: Some(name.clone()),
                                        pstl_adr: None,
                                    })
                                } else {
                                    None
                                },
                                dbtr_acct: if tx.debit_credit == DebitCredit::Credit {
                                    tx.counterparty_account.as_ref().map(|acc| AccountXml {
                                        id: AccountIdXml {
                                            iban: Some(acc.clone()),
                                            othr: None,
                                        },
                                    })
                                } else {
                                    None
                                },
                                cdtr: if tx.debit_credit == DebitCredit::Debit {
                                    tx.counterparty_name.as_ref().map(|name| PartyXml {
                                        nm: Some(name.clone()),
                                        pstl_adr: None,
                                    })
                                } else {
                                    None
                                },
                                cdtr_acct: if tx.debit_credit == DebitCredit::Debit {
                                    tx.counterparty_account.as_ref().map(|acc| AccountXml {
                                        id: AccountIdXml {
                                            iban: Some(acc.clone()),
                                            othr: None,
                                        },
                                    })
                                } else {
                                    None
                                },
                            })
                        } else {
                            None
                        },
                        rltd_agts: None,
                        rmt_inf: if !tx.description.is_empty() {
                            Some(RemittanceInformationXml {
                                ustrd: Some(tx.description.clone()),
                                strd: None,
                            })
                        } else {
                            None
                        },
                        rltd_dts: None,
                        addtl_tx_inf: tx.additional_info.clone(),
                    }),
                    btch: None,
                }),
            }
        }).collect();

        Document {
            bk_to_cstmr_stmt: BankToCustomerStatementXml {
                grp_hdr: GroupHeaderXml {
                    msg_id: stmt.statement_id.clone(),
                    cre_dt_tm: stmt.creation_date
                        .as_ref()
                        .map(format_date_time)
                        .unwrap_or_else(|| format_date_time(&chrono::Utc::now().date_naive())),
                },
                stmt: StatementXml {
                    id: stmt.statement_id.clone(),
                    elctrnic_seq_nb: stmt.sequence_number.as_ref().and_then(|s| s.parse().ok()),
                    lgl_seq_nb: None,
                    cre_dt_tm: stmt.creation_date.as_ref().map(format_date_time),
                    fr_to_dt: if stmt.from_date.is_some() || stmt.to_date.is_some() {
                        Some(FromToDateXml {
                            fr_dt_tm: stmt.from_date.as_ref().map(format_date_time),
                            to_dt_tm: stmt.to_date.as_ref().map(format_date_time),
                        })
                    } else {
                        None
                    },
                    acct: AccountInfoXml {
                        id: AccountIdXml {
                            iban: Some(stmt.account.clone()),
                            othr: None,
                        },
                        ccy: stmt.currency.clone(),
                        nm: stmt.account_holder.clone(),
                        ownr: None,
                        svcr: None,
                    },
                    bal: balances,
                    txs_summry: None,
                    ntry: entries,
                },
            },
        }
    }
}

// XML structure definitions
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "Document")]
struct Document {
    #[serde(rename = "BkToCstmrStmt")]
    bk_to_cstmr_stmt: BankToCustomerStatementXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct BankToCustomerStatementXml {
    #[serde(rename = "GrpHdr")]
    grp_hdr: GroupHeaderXml,
    #[serde(rename = "Stmt")]
    stmt: StatementXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct GroupHeaderXml {
    #[serde(rename = "MsgId")]
    msg_id: String,
    #[serde(rename = "CreDtTm")]
    cre_dt_tm: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct StatementXml {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "ElctrncSeqNb", skip_serializing_if = "Option::is_none")]
    elctrnic_seq_nb: Option<u32>,
    #[serde(rename = "LglSeqNb", skip_serializing_if = "Option::is_none")]
    lgl_seq_nb: Option<u32>,
    #[serde(rename = "CreDtTm", skip_serializing_if = "Option::is_none")]
    cre_dt_tm: Option<String>,
    #[serde(rename = "FrToDt", skip_serializing_if = "Option::is_none")]
    fr_to_dt: Option<FromToDateXml>,
    #[serde(rename = "Acct")]
    acct: AccountInfoXml,
    #[serde(rename = "Bal", default)]
    bal: Vec<BalanceXml>,
    #[serde(rename = "TxsSummry", skip_serializing_if = "Option::is_none")]
    txs_summry: Option<TransactionsSummaryXml>,
    #[serde(rename = "Ntry", default)]
    ntry: Vec<EntryXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FromToDateXml {
    #[serde(rename = "FrDtTm", skip_serializing_if = "Option::is_none")]
    fr_dt_tm: Option<String>,
    #[serde(rename = "ToDtTm", skip_serializing_if = "Option::is_none")]
    to_dt_tm: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AccountInfoXml {
    #[serde(rename = "Id")]
    id: AccountIdXml,
    #[serde(rename = "Ccy")]
    ccy: String,
    #[serde(rename = "Nm", skip_serializing_if = "Option::is_none")]
    nm: Option<String>,
    #[serde(rename = "Ownr", skip_serializing_if = "Option::is_none")]
    ownr: Option<OwnerXml>,
    #[serde(rename = "Svcr", skip_serializing_if = "Option::is_none")]
    svcr: Option<ServicerXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AccountIdXml {
    #[serde(rename = "IBAN", skip_serializing_if = "Option::is_none")]
    iban: Option<String>,
    #[serde(rename = "Othr", skip_serializing_if = "Option::is_none")]
    othr: Option<OtherAccountIdXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OtherAccountIdXml {
    #[serde(rename = "Id")]
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct OwnerXml {
    #[serde(rename = "Nm", skip_serializing_if = "Option::is_none")]
    nm: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ServicerXml {
    #[serde(rename = "FinInstnId")]
    fin_instn_id: FinancialInstitutionIdXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct FinancialInstitutionIdXml {
    #[serde(rename = "BIC", skip_serializing_if = "Option::is_none")]
    bic: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct BalanceXml {
    #[serde(rename = "Tp")]
    tp: BalanceTypeXml,
    #[serde(rename = "Amt")]
    amt: AmountXml,
    #[serde(rename = "CdtDbtInd")]
    cdt_dbt_ind: String,
    #[serde(rename = "Dt")]
    dt: DateXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct BalanceTypeXml {
    #[serde(rename = "CdOrPrtry")]
    cd_or_prtry: CodeOrProprietaryXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct CodeOrProprietaryXml {
    #[serde(rename = "Cd")]
    cd: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct AmountXml {
    #[serde(rename = "$value")]
    value: String,
    #[serde(rename = "@Ccy", skip_serializing_if = "Option::is_none")]
    ccy: Option<String>,
    #[serde(rename = "Ccy", skip_serializing_if = "Option::is_none")]
    ccy_alt: Option<String>,
}

impl AmountXml {
    fn ccy(&self) -> Option<String> {
        self.ccy.clone().or_else(|| self.ccy_alt.clone())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct DateXml {
    #[serde(rename = "Dt", skip_serializing_if = "Option::is_none")]
    dt: Option<String>,
    #[serde(rename = "DtTm", skip_serializing_if = "Option::is_none")]
    dt_tm: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TransactionsSummaryXml {
    #[serde(rename = "TtlNtries", skip_serializing_if = "Option::is_none")]
    ttl_ntries: Option<TotalEntriesXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TotalEntriesXml {
    #[serde(rename = "NbOfNtries")]
    nb_of_ntries: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct EntryXml {
    #[serde(rename = "NtryRef", skip_serializing_if = "Option::is_none")]
    ntry_ref: Option<String>,
    #[serde(rename = "Amt")]
    amt: AmountXml,
    #[serde(rename = "CdtDbtInd")]
    cdt_dbt_ind: String,
    #[serde(rename = "Sts")]
    sts: String,
    #[serde(rename = "BookgDt", skip_serializing_if = "Option::is_none")]
    bookg_dt: Option<DateXml>,
    #[serde(rename = "ValDt", skip_serializing_if = "Option::is_none")]
    val_dt: Option<DateXml>,
    #[serde(rename = "AcctSvcrRef", skip_serializing_if = "Option::is_none")]
    acct_svcr_ref: Option<String>,
    #[serde(rename = "BkTxCd", skip_serializing_if = "Option::is_none")]
    bk_tx_cd: Option<BankTransactionCodeXml>,
    #[serde(rename = "NtryDtls", skip_serializing_if = "Option::is_none")]
    ntry_dtls: Option<EntryDetailsXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct BankTransactionCodeXml {
    #[serde(rename = "Domn", skip_serializing_if = "Option::is_none")]
    domn: Option<DomainXml>,
    #[serde(rename = "Prtry", skip_serializing_if = "Option::is_none")]
    prtry: Option<ProprietaryCodeXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DomainXml {
    #[serde(rename = "Cd")]
    cd: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ProprietaryCodeXml {
    #[serde(rename = "Cd")]
    cd: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct EntryDetailsXml {
    #[serde(rename = "Btch", skip_serializing_if = "Option::is_none")]
    btch: Option<BatchXml>,
    #[serde(rename = "TxDtls", skip_serializing_if = "Option::is_none")]
    tx_dtls: Option<TransactionDetailsXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct BatchXml {
    #[serde(rename = "NbOfTxs")]
    nb_of_txs: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TransactionDetailsXml {
    #[serde(rename = "Refs", skip_serializing_if = "Option::is_none")]
    refs: Option<ReferencesXml>,
    #[serde(rename = "AmtDtls", skip_serializing_if = "Option::is_none")]
    amt_dtls: Option<AmountDetailsXml>,
    #[serde(rename = "RltdPties", skip_serializing_if = "Option::is_none")]
    rltd_pties: Option<RelatedPartiesXml>,
    #[serde(rename = "RltdAgts", skip_serializing_if = "Option::is_none")]
    rltd_agts: Option<RelatedAgentsXml>,
    #[serde(rename = "RmtInf", skip_serializing_if = "Option::is_none")]
    rmt_inf: Option<RemittanceInformationXml>,
    #[serde(rename = "RltdDts", skip_serializing_if = "Option::is_none")]
    rltd_dts: Option<RelatedDatesXml>,
    #[serde(rename = "AddtlTxInf", skip_serializing_if = "Option::is_none")]
    addtl_tx_inf: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ReferencesXml {
    #[serde(rename = "EndToEndId", skip_serializing_if = "Option::is_none")]
    end_to_end_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AmountDetailsXml {
    #[serde(rename = "TxAmt", skip_serializing_if = "Option::is_none")]
    tx_amt: Option<AmountXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RelatedPartiesXml {
    #[serde(rename = "Dbtr", skip_serializing_if = "Option::is_none")]
    dbtr: Option<PartyXml>,
    #[serde(rename = "DbtrAcct", skip_serializing_if = "Option::is_none")]
    dbtr_acct: Option<AccountXml>,
    #[serde(rename = "Cdtr", skip_serializing_if = "Option::is_none")]
    cdtr: Option<PartyXml>,
    #[serde(rename = "CdtrAcct", skip_serializing_if = "Option::is_none")]
    cdtr_acct: Option<AccountXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PartyXml {
    #[serde(rename = "Nm", skip_serializing_if = "Option::is_none")]
    nm: Option<String>,
    #[serde(rename = "PstlAdr", skip_serializing_if = "Option::is_none")]
    pstl_adr: Option<PostalAddressXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PostalAddressXml {
    #[serde(rename = "Ctry", skip_serializing_if = "Option::is_none")]
    ctry: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AccountXml {
    #[serde(rename = "Id")]
    id: AccountIdXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct RelatedAgentsXml {
    #[serde(rename = "DbtrAgt", skip_serializing_if = "Option::is_none")]
    dbtr_agt: Option<AgentXml>,
    #[serde(rename = "CdtrAgt", skip_serializing_if = "Option::is_none")]
    cdtr_agt: Option<AgentXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AgentXml {
    #[serde(rename = "FinInstnId")]
    fin_instn_id: FinancialInstitutionIdXml,
}

#[derive(Debug, Deserialize, Serialize)]
struct RemittanceInformationXml {
    #[serde(rename = "Ustrd", skip_serializing_if = "Option::is_none")]
    ustrd: Option<String>,
    #[serde(rename = "Strd", skip_serializing_if = "Option::is_none")]
    strd: Option<StructuredRemittanceXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct StructuredRemittanceXml {
    #[serde(rename = "CdtrRefInf", skip_serializing_if = "Option::is_none")]
    cdtr_ref_inf: Option<CreditorReferenceXml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CreditorReferenceXml {
    #[serde(rename = "Ref", skip_serializing_if = "Option::is_none")]
    ref_val: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RelatedDatesXml {
    #[serde(rename = "AccptncDtTm", skip_serializing_if = "Option::is_none")]
    accptnc_dt_tm: Option<String>,
}

// Helper functions for date parsing and formatting
fn parse_camt_date(date_str: &str) -> Result<NaiveDate> {
    // Try different date formats
    // ISO 8601 with time: 2023-04-20T23:24:31
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.date());
    }

    // ISO 8601 date only: 2023-04-20
    parse_date_only(date_str)
}

fn parse_date_only(date_str: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| Error::InvalidDate(date_str.to_string()))
}

fn format_date_time(date: &NaiveDate) -> String {
    format!("{}T00:00:00", date.format("%Y-%m-%d"))
}

fn format_date_only(date: &NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_date() {
        let date = parse_camt_date("2023-04-20T23:24:31").unwrap();
        assert_eq!(date.year(), 2023);
        assert_eq!(date.month(), 4);
        assert_eq!(date.day(), 20);
    }
}
