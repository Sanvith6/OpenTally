# Workstream 4: Inventory Engine Specification

## 1. Inventory Entity Hierarchy & Masters
TallyPrime maintains a multi-dimensional inventory subsystem seamlessly linked to accounting ledgers:
1. **Stock Groups**: Logical grouping of items (e.g., "Electronics", "Finished Goods", "Raw Materials"). Can have sub-groups. Supports shared GST rate inheritance.
2. **Stock Categories**: Parallel cross-classification across groups (e.g., "Premium Range", "Budget Range").
3. **Stock Items**: The primary physical product entity.
   - Master fields: Name, Part Number, Description, Stock Group, Stock Category, Unit of Measure, Opening Quantity, Opening Rate, Opening Value, Costing Method, Market Valuation Method.
4. **Units of Measurement (UoM)**:
   - **Simple Units**: Symbol (e.g., `Nos`, `Kgs`, `Boxes`), Formal Name (e.g., `Numbers`), Number of Decimal Places (0 to 4).
   - **Compound Units**: Relationship between two simple units (e.g., `1 Box = 24 Nos`).
   - **Alternative Units**: Secondary unit with dynamic conversion ratio per transaction (e.g., `100 Bags` where total weight is `5050 Kgs`).
5. **Godowns / Locations**: Physical storage points. Supports hierarchical godowns (e.g., "Warehouse North > Floor 1 > Rack B"). Default: "Main Location".
6. **Batches & Expiry Dates**:
   - Enables tracking items by Batch / Lot Number.
   - Captures **Manufacturing Date** and **Expiry Date**.
   - Prevents or warns against selling expired batches in Sales vouchers.

## 2. Stock Valuation Methods
TallyPrime calculates stock closing value dynamically across several valuation algorithms:
- **FIFO (First In First Out)**: Oldest inventory lots are consumed first; remaining stock valued at newest purchase rates.
- **LIFO (Last In First Out)**: Newest lots consumed first (legacy; disallowed under Ind-AS 2 / IFRS).
- **Monthly Avg Cost**: Weighted average unit cost computed monthly.
- **At Zero Cost**: Item value forced to zero in balance sheet.
- **Avg Cost (Running Average)**: Cumulative total purchase value divided by cumulative quantity.
- **Last Purchase Rate**: Entire closing stock valued at the unit rate of the most recent purchase invoice.
- **Standard Cost**: User-defined fixed standard valuation cost.

## 3. Accounting & Inventory Integration Invariants
- Configuration: `Integrate Accounts and Inventory = Yes/No`.
- When `Yes`: The closing balance of the `Stock-in-Hand` group on the Balance Sheet is automatically calculated by multiplying current physical stock quantities by their valuation rates across all stock items.
- When `No`: The `Stock-in-Hand` ledger behaves as a pure financial ledger whose closing balance is manually entered via closing balance journal entries or periodic ledger alterations.
