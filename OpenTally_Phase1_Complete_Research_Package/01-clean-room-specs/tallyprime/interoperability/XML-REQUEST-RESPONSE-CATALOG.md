# OpenTally XML Request/Response Catalog

This catalog documents standard, sanitized XML payloads for core master import, voucher ingestion, and data export operations.

## 1. Master Import: Ledger Creation Request

### Request Payload (`POST http://localhost:9000/`)
```xml
<ENVELOPE>
    <HEADER>
        <TALLYREQUEST>Import Data</TALLYREQUEST>
    </HEADER>
    <BODY>
        <IMPORTDATA>
            <REQUESTDESC>
                <REPORTNAME>All Masters</REPORTNAME>
                <STATICVARIABLES>
                    <SVCURRENTCOMPANY>National Synthetic Trade Enterprises</SVCURRENTCOMPANY>
                </STATICVARIABLES>
            </REQUESTDESC>
            <REQUESTDATA>
                <TALLYMESSAGE xmlns:UDF="TallyUDF">
                    <LEDGER NAME="Synthetic Customer Alpha" ACTION="Create">
                        <NAME>Synthetic Customer Alpha</NAME>
                        <PARENT>Sundry Debtors</PARENT>
                        <OPENINGBALANCE>-25000.00</OPENINGBALANCE>
                        <ISBILLWISEON>Yes</ISBILLWISEON>
                        <COUNTRYOFRESIDENCE>India</COUNTRYOFRESIDENCE>
                        <LEDSTATENAME>Karnataka</LEDSTATENAME>
                        <PARTYGSTIN>29ABCDE1234F1Z5</PARTYGSTIN>
                        <GSTREGISTRATIONTYPE>Regular</GSTREGISTRATIONTYPE>
                    </LEDGER>
                </TALLYMESSAGE>
            </REQUESTDATA>
        </IMPORTDATA>
    </BODY>
</ENVELOPE>
```

### Expected Response
```xml
<RESPONSE>
    <CREATED>1</CREATED>
    <ALTERED>0</ALTERED>
    <DELETED>0</DELETED>
    <LASTVCHID>0</LASTVCHID>
    <LASTMID>1042</LASTMID>
    <COMBINEDERRORS>0</COMBINEDERRORS>
</RESPONSE>
```

## 2. Voucher Import: Sales Voucher with GST Request

### Request Payload
```xml
<ENVELOPE>
    <HEADER>
        <TALLYREQUEST>Import Data</TALLYREQUEST>
    </HEADER>
    <BODY>
        <IMPORTDATA>
            <REQUESTDESC>
                <REPORTNAME>Vouchers</REPORTNAME>
                <STATICVARIABLES>
                    <SVCURRENTCOMPANY>National Synthetic Trade Enterprises</SVCURRENTCOMPANY>
                </STATICVARIABLES>
            </REQUESTDESC>
            <REQUESTDATA>
                <TALLYMESSAGE xmlns:UDF="TallyUDF">
                    <VOUCHER VCHTYPE="Sales" ACTION="Create" OBJVIEW="Invoice Voucher View">
                        <DATE>20240510</DATE>
                        <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME>
                        <VOUCHERNUMBER>INV-2425-001</VOUCHERNUMBER>
                        <PARTYLEDGERNAME>Synthetic Customer Alpha</PARTYLEDGERNAME>
                        <PERSISTEDVIEW>Invoice Voucher View</PERSISTEDVIEW>
                        <PLACEOFSUPPLY>Karnataka</PLACEOFSUPPLY>
                        <!-- Debtor Posting (Debit is Negative in Tally XML) -->
                        <ALLLEDGERENTRIES.LIST>
                            <LEDGERNAME>Synthetic Customer Alpha</LEDGERNAME>
                            <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
                            <AMOUNT>-11800.00</AMOUNT>
                            <BILLALLOCATIONS.LIST>
                                <NAME>INV-2425-001</NAME>
                                <BILLTYPE>New Ref</BILLTYPE>
                                <AMOUNT>-11800.00</AMOUNT>
                            </BILLALLOCATIONS.LIST>
                        </ALLLEDGERENTRIES.LIST>
                        <!-- Sales Posting (Credit is Positive) -->
                        <ALLLEDGERENTRIES.LIST>
                            <LEDGERNAME>Local Sales 18%</LEDGERNAME>
                            <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
                            <AMOUNT>10000.00</AMOUNT>
                        </ALLLEDGERENTRIES.LIST>
                        <!-- CGST Posting -->
                        <ALLLEDGERENTRIES.LIST>
                            <LEDGERNAME>CGST</LEDGERNAME>
                            <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
                            <AMOUNT>900.00</AMOUNT>
                        </ALLLEDGERENTRIES.LIST>
                        <!-- SGST Posting -->
                        <ALLLEDGERENTRIES.LIST>
                            <LEDGERNAME>SGST</LEDGERNAME>
                            <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
                            <AMOUNT>900.00</AMOUNT>
                        </ALLLEDGERENTRIES.LIST>
                    </VOUCHER>
                </TALLYMESSAGE>
            </REQUESTDATA>
        </IMPORTDATA>
    </BODY>
</ENVELOPE>
```

### Expected Response
```xml
<RESPONSE>
    <CREATED>1</CREATED>
    <ALTERED>0</ALTERED>
    <DELETED>0</DELETED>
    <LASTVCHID>2015</LASTVCHID>
    <LASTMID>0</LASTMID>
    <COMBINEDERRORS>0</COMBINEDERRORS>
</RESPONSE>
```

## 3. Critical XML Data Interchange Invariants
1. **Debit vs Credit Sign Convention**: In Tally XML payloads:
   - **Debits** are represented by `<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>` and a negative numeric value (e.g., `-11800.00`).
   - **Credits** are represented by `<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>` and a positive numeric value (e.g., `10000.00`).
   - `SUM(AMOUNT)` across all `<ALLLEDGERENTRIES.LIST>` must equal `0.00`.
2. **Date Format**: Standard `YYYYMMDD` string (e.g., `20240510` for May 10, 2024).
