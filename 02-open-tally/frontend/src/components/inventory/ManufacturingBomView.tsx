import { useCallback, useEffect, useState } from 'react';
import {
  BomDetail,
  Company,
  CreateBomComponentInput,
  Godown,
  Ledger,
  ManufacturingJournalResult,
  StockItem,
} from '../../types';
import {
  createBom,
  executeManufacturingJournal,
  listBoms,
  listGodowns,
  listLedgers,
  listStockItems,
} from '../../services/api';
import '../reports/Reports.css';
import { ManufacturingAdvancedPanel } from './ManufacturingAdvancedPanel';
import { isPositiveFactoryDecimal, previewFactoryConsumption } from '../../utils/factoryDecimal';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

interface OverheadRow {
  id: string;
  overhead_ledger_id: string;
  amount: string;
  description: string;
}

export function ManufacturingBomView({ activeCompany, onBack }: Props) {
  const [activeTab, setActiveTab] = useState<'boms' | 'produce' | 'advanced'>('boms');
  const [boms, setBoms] = useState<BomDetail[]>([]);
  const [stockItems, setStockItems] = useState<StockItem[]>([]);
  const [godowns, setGodowns] = useState<Godown[]>([]);
  const [ledgers, setLedgers] = useState<Ledger[]>([]);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // Create BOM Form State
  const [showCreateBom, setShowCreateBom] = useState(false);
  const [bomFinishedItem, setBomFinishedItem] = useState('');
  const [bomName, setBomName] = useState('');
  const [bomBaseQty, setBomBaseQty] = useState('1');
  const [bomDefaultGodown, setBomDefaultGodown] = useState('');
  const [bomComponents, setBomComponents] = useState<CreateBomComponentInput[]>([
    { raw_stock_item_id: '', source_godown_id: null, required_qty: '1' },
  ]);

  // Manufacturing Journal Form State
  const [selectedBomId, setSelectedBomId] = useState('');
  const [productionDate, setProductionDate] = useState(
    new Date().toISOString().slice(0, 10)
  );
  const [producedQty, setProducedQty] = useState('10');
  const [destGodownId, setDestGodownId] = useState('');
  const [batchNumber, setBatchNumber] = useState('');
  const [narration, setNarration] = useState('');
  const [overheads, setOverheads] = useState<OverheadRow[]>([]);

  // Production Execution Result State
  const [prodResult, setProdResult] =
    useState<ManufacturingJournalResult | null>(null);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    try {
      setLoading(true);
      setError(null);
      const [bList, items, gList, lList] = await Promise.all([
        listBoms(activeCompany.id),
        listStockItems(activeCompany.id),
        listGodowns(activeCompany.id),
        listLedgers(activeCompany.id),
      ]);
      setBoms(bList);
      setStockItems(items);
      setGodowns(gList);
      setLedgers(lList);

      if (bList.length > 0 && !selectedBomId) {
        setSelectedBomId(bList[0].id);
      }
      if (gList.length > 0 && !destGodownId) {
        setDestGodownId(gList[0].id);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to load BOM data');
    } finally {
      setLoading(false);
    }
  }, [activeCompany, selectedBomId, destGodownId]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Handle adding component row in BOM creator
  const addComponentRow = () => {
    setBomComponents([
      ...bomComponents,
      { raw_stock_item_id: '', source_godown_id: null, required_qty: '1' },
    ]);
  };

  const removeComponentRow = (index: number) => {
    if (bomComponents.length <= 1) return;
    setBomComponents(bomComponents.filter((_, i) => i !== index));
  };

  const updateComponentRow = (
    index: number,
    field: keyof CreateBomComponentInput,
    val: string | null
  ) => {
    const updated = [...bomComponents];
    updated[index] = { ...updated[index], [field]: val };
    setBomComponents(updated);
  };

  // Handle saving new BOM
  const handleSaveBom = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;

    if (!bomFinishedItem) {
      setError('Please select a finished goods item');
      return;
    }
    if (!bomName.trim()) {
      setError('BOM name is required');
      return;
    }
    if (!isPositiveFactoryDecimal(bomBaseQty)) {
      setError('Base output quantity must be greater than zero');
      return;
    }

    for (let i = 0; i < bomComponents.length; i++) {
      const c = bomComponents[i];
      if (!c.raw_stock_item_id) {
        setError(`Please select a raw material for component #${i + 1}`);
        return;
      }
      if (c.raw_stock_item_id === bomFinishedItem) {
        setError(
          `Self-referential component in line #${i + 1}: Component cannot be the finished item itself`
        );
        return;
      }
      if (!isPositiveFactoryDecimal(c.required_qty)) {
        setError(`Required quantity in component #${i + 1} must be positive`);
        return;
      }
    }

    try {
      setLoading(true);
      setError(null);
      const created = await createBom({
        company_id: activeCompany.id,
        finished_stock_item_id: bomFinishedItem,
        bom_name: bomName.trim(),
        base_output_qty: bomBaseQty,
        default_godown_id: bomDefaultGodown || null,
        components: bomComponents,
      });

      setMessage(
        `Bill of Materials '${created.bom_name}' created successfully with ${created.components.length} components.`
      );
      setShowCreateBom(false);
      setBomName('');
      setBomComponents([
        { raw_stock_item_id: '', source_godown_id: null, required_qty: '1' },
      ]);
      await loadData();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to create Bill of Materials');
    } finally {
      setLoading(false);
    }
  };

  // Overhead management
  const addOverheadRow = () => {
    setOverheads([
      ...overheads,
      {
        id: Math.random().toString(36).substring(2, 9),
        overhead_ledger_id: '',
        amount: '0.00',
        description: '',
      },
    ]);
  };

  const removeOverheadRow = (id: string) => {
    setOverheads(overheads.filter((o) => o.id !== id));
  };

  const updateOverheadRow = (
    id: string,
    field: keyof OverheadRow,
    val: string
  ) => {
    setOverheads(
      overheads.map((o) => (o.id === id ? { ...o, [field]: val } : o))
    );
  };

  // Active BOM for production preview
  const currentBom = boms.find((b) => b.id === selectedBomId);

  // Live scaling calculation
  const scaleRatio = previewFactoryConsumption(producedQty, '1', currentBom?.base_output_qty || '1');

  // Handle production execution
  const handleExecuteProduction = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    if (!selectedBomId) {
      setError('Please select a Bill of Materials recipe');
      return;
    }
    if (!isPositiveFactoryDecimal(producedQty)) {
      setError('Produced quantity must be greater than zero');
      return;
    }
    if (!destGodownId) {
      setError('Please select a destination godown');
      return;
    }

    try {
      setLoading(true);
      setError(null);
      setProdResult(null);

      const overheadPayload = overheads
        .filter((o) => o.overhead_ledger_id && isPositiveFactoryDecimal(o.amount))
        .map((o) => ({
          overhead_ledger_id: o.overhead_ledger_id,
          amount: o.amount,
          description: o.description || null,
        }));

      const res = await executeManufacturingJournal({
        company_id: activeCompany.id,
        bom_id: selectedBomId,
        production_date: productionDate,
        produced_qty: producedQty,
        destination_godown_id: destGodownId,
        batch_number: batchNumber.trim() || null,
        additional_overheads: overheadPayload,
        narration: narration.trim() || null,
      });

      setProdResult(res);
      setMessage(
        `Production completed successfully! Produced ${res.produced_qty} units at exact landed unit cost ₹${res.landed_unit_cost}.`
      );
      setOverheads([]);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to execute manufacturing journal');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="report-container"
      style={{ padding: '20px', maxWidth: '1080px', margin: '0 auto' }}
    >
      {/* Header & Tabs */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '16px',
        }}
      >
        <div>
          <h1
            style={{
              margin: 0,
              fontSize: '20px',
              color: '#004d40',
              fontWeight: 'bold',
            }}
          >
            Manufacturing & Bill of Materials (BOM)
          </h1>
          <div style={{ fontSize: '13px', color: '#666', marginTop: '4px' }}>
            Company:{' '}
            <strong>{activeCompany?.name || 'No Company Selected'}</strong>
          </div>
        </div>
        <button
          onClick={onBack}
          style={{
            padding: '6px 14px',
            backgroundColor: '#f5f5f5',
            border: '1px solid #ccc',
            borderRadius: '4px',
            cursor: 'pointer',
            fontSize: '13px',
          }}
        >
          Back to Gateway (Esc)
        </button>
      </div>

      <div style={{ display: 'flex', gap: '8px', marginBottom: '16px' }}>
        <button
          onClick={() => {
            setActiveTab('boms');
            setProdResult(null);
          }}
          style={{
            padding: '8px 18px',
            backgroundColor: activeTab === 'boms' ? '#004d40' : '#e0e0e0',
            color: activeTab === 'boms' ? '#fff' : '#333',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
            fontWeight: 600,
            fontSize: '13px',
          }}
        >
          1. Recipes (Bill of Materials)
        </button>
        <button
          onClick={() => setActiveTab('produce')}
          style={{
            padding: '8px 18px',
            backgroundColor: activeTab === 'produce' ? '#004d40' : '#e0e0e0',
            color: activeTab === 'produce' ? '#fff' : '#333',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer',
            fontWeight: 600,
            fontSize: '13px',
          }}
        >
          2. Manufacturing Stock Journal (Production)
        </button>
        <button className="ot-report-action" aria-pressed={activeTab === 'advanced'} onClick={() => setActiveTab('advanced')}>3. Multi-Level BOM &amp; Scrap Recovery</button>
      </div>

      {message && (
        <div
          style={{
            padding: '10px 14px',
            backgroundColor: '#e8f5e9',
            color: '#2e7d32',
            borderRadius: '4px',
            marginBottom: '16px',
            fontSize: '13px',
          }}
        >
          {message}
        </div>
      )}

      {error && (
        <div
          style={{
            padding: '10px 14px',
            backgroundColor: '#ffebee',
            color: '#c62828',
            borderRadius: '4px',
            marginBottom: '16px',
            fontSize: '13px',
          }}
        >
          {error}
        </div>
      )}

      {activeTab === 'advanced' && activeCompany && <>
        <label className="ot-report-field">BOM for explosion<select value={selectedBomId} onChange={(event) => setSelectedBomId(event.target.value)}><option value="">Select BOM</option>{boms.map((bom) => <option key={bom.id} value={bom.id}>{bom.bom_name}</option>)}</select></label>
        <ManufacturingAdvancedPanel companyId={activeCompany.id} bomId={selectedBomId} />
      </>}
      {/* TAB 1: BOM RECIPES */}
      {activeTab === 'boms' && (
        <div>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '14px',
            }}
          >
            <h2
              style={{
                margin: 0,
                fontSize: '16px',
                color: '#004d40',
                fontWeight: 'bold',
              }}
            >
              Configured Recipes
            </h2>
            <button
              onClick={() => setShowCreateBom(!showCreateBom)}
              style={{
                padding: '6px 14px',
                backgroundColor: showCreateBom ? '#757575' : '#004d40',
                color: '#fff',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '13px',
                fontWeight: 600,
              }}
            >
              {showCreateBom ? 'Cancel' : '+ Define New BOM Recipe'}
            </button>
          </div>

          {/* Create BOM Form */}
          {showCreateBom && (
            <div
              style={{
                backgroundColor: '#fff',
                border: '1px solid #b2dfdb',
                borderRadius: '6px',
                padding: '18px',
                marginBottom: '20px',
                boxShadow: '0 2px 5px rgba(0,0,0,0.06)',
              }}
            >
              <h3
                style={{
                  margin: '0 0 14px 0',
                  fontSize: '15px',
                  color: '#004d40',
                }}
              >
                Create Bill of Materials Recipe
              </h3>
              <form onSubmit={handleSaveBom}>
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                    gap: '14px',
                    marginBottom: '16px',
                  }}
                >
                  <div>
                    <label
                      style={{
                        display: 'block',
                        fontSize: '12px',
                        fontWeight: 600,
                        marginBottom: '4px',
                      }}
                    >
                      Finished Stock Item *
                    </label>
                    <select
                      value={bomFinishedItem}
                      onChange={(e) => setBomFinishedItem(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '6px 10px',
                        fontSize: '13px',
                        border: '1px solid #ccc',
                        borderRadius: '4px',
                      }}
                    >
                      <option value="">-- Select Finished Item --</option>
                      {stockItems.map((si) => (
                        <option key={si.id} value={si.id}>
                          {si.name}
                        </option>
                      ))}
                    </select>
                  </div>

                  <div>
                    <label
                      style={{
                        display: 'block',
                        fontSize: '12px',
                        fontWeight: 600,
                        marginBottom: '4px',
                      }}
                    >
                      BOM Name *
                    </label>
                    <input
                      type="text"
                      value={bomName}
                      onChange={(e) => setBomName(e.target.value)}
                      placeholder="e.g. Standard Recipe / Batch of 100"
                      style={{
                        width: '100%',
                        padding: '6px 10px',
                        fontSize: '13px',
                        border: '1px solid #ccc',
                        borderRadius: '4px',
                        boxSizing: 'border-box',
                      }}
                    />
                  </div>

                  <div>
                    <label
                      style={{
                        display: 'block',
                        fontSize: '12px',
                        fontWeight: 600,
                        marginBottom: '4px',
                      }}
                    >
                      Base Output Quantity *
                    </label>
                    <input
                      type="text"
                      value={bomBaseQty}
                      onChange={(e) => setBomBaseQty(e.target.value)}
                      placeholder="e.g. 1"
                      style={{
                        width: '100%',
                        padding: '6px 10px',
                        fontSize: '13px',
                        border: '1px solid #ccc',
                        borderRadius: '4px',
                        boxSizing: 'border-box',
                      }}
                    />
                  </div>

                  <div>
                    <label
                      style={{
                        display: 'block',
                        fontSize: '12px',
                        fontWeight: 600,
                        marginBottom: '4px',
                      }}
                    >
                      Default Godown (Optional)
                    </label>
                    <select
                      value={bomDefaultGodown}
                      onChange={(e) => setBomDefaultGodown(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '6px 10px',
                        fontSize: '13px',
                        border: '1px solid #ccc',
                        borderRadius: '4px',
                      }}
                    >
                      <option value="">-- None --</option>
                      {godowns.map((g) => (
                        <option key={g.id} value={g.id}>
                          {g.name}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>

                {/* Component Table */}
                <h4
                  style={{
                    margin: '0 0 8px 0',
                    fontSize: '13px',
                    color: '#333',
                  }}
                >
                  Raw Material Components
                </h4>
                <table
                  style={{
                    width: '100%',
                    borderCollapse: 'collapse',
                    fontSize: '13px',
                    marginBottom: '14px',
                  }}
                >
                  <thead>
                    <tr
                      style={{
                        backgroundColor: '#f5f5f5',
                        borderBottom: '1px solid #ddd',
                        textAlign: 'left',
                      }}
                    >
                      <th style={{ padding: '6px 8px' }}>#</th>
                      <th style={{ padding: '6px 8px' }}>
                        Raw Material Item *
                      </th>
                      <th style={{ padding: '6px 8px' }}>
                        Source Godown (Optional)
                      </th>
                      <th style={{ padding: '6px 8px' }}>Required Qty *</th>
                      <th style={{ padding: '6px 8px', textAlign: 'right' }}>
                        Action
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {bomComponents.map((comp, idx) => (
                      <tr key={idx} style={{ borderBottom: '1px solid #eee' }}>
                        <td style={{ padding: '6px 8px', color: '#888' }}>
                          {idx + 1}
                        </td>
                        <td style={{ padding: '6px 8px' }}>
                          <select
                            value={comp.raw_stock_item_id}
                            onChange={(e) =>
                              updateComponentRow(
                                idx,
                                'raw_stock_item_id',
                                e.target.value
                              )
                            }
                            style={{
                              width: '100%',
                              padding: '4px 8px',
                              fontSize: '12px',
                              border: '1px solid #ccc',
                              borderRadius: '3px',
                            }}
                          >
                            <option value="">-- Select Raw Item --</option>
                            {stockItems
                              .filter((si) => si.id !== bomFinishedItem)
                              .map((si) => (
                                <option key={si.id} value={si.id}>
                                  {si.name}
                                </option>
                              ))}
                          </select>
                        </td>
                        <td style={{ padding: '6px 8px' }}>
                          <select
                            value={comp.source_godown_id || ''}
                            onChange={(e) =>
                              updateComponentRow(
                                idx,
                                'source_godown_id',
                                e.target.value || null
                              )
                            }
                            style={{
                              width: '100%',
                              padding: '4px 8px',
                              fontSize: '12px',
                              border: '1px solid #ccc',
                              borderRadius: '3px',
                            }}
                          >
                            <option value="">-- Any / Default --</option>
                            {godowns.map((g) => (
                              <option key={g.id} value={g.id}>
                                {g.name}
                              </option>
                            ))}
                          </select>
                        </td>
                        <td style={{ padding: '6px 8px' }}>
                          <input
                            type="text"
                            value={comp.required_qty}
                            onChange={(e) =>
                              updateComponentRow(
                                idx,
                                'required_qty',
                                e.target.value
                              )
                            }
                            style={{
                              width: '90px',
                              padding: '4px 8px',
                              fontSize: '12px',
                              border: '1px solid #ccc',
                              borderRadius: '3px',
                            }}
                          />
                        </td>
                        <td style={{ padding: '6px 8px', textAlign: 'right' }}>
                          <button
                            type="button"
                            onClick={() => removeComponentRow(idx)}
                            disabled={bomComponents.length <= 1}
                            style={{
                              padding: '3px 8px',
                              backgroundColor: '#ffebee',
                              color: '#c62828',
                              border: '1px solid #ffcdd2',
                              borderRadius: '3px',
                              cursor:
                                bomComponents.length <= 1
                                  ? 'not-allowed'
                                  : 'pointer',
                              fontSize: '11px',
                            }}
                          >
                            Remove
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>

                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                  }}
                >
                  <button
                    type="button"
                    onClick={addComponentRow}
                    style={{
                      padding: '5px 12px',
                      backgroundColor: '#e0f2f1',
                      color: '#004d40',
                      border: '1px solid #b2dfdb',
                      borderRadius: '4px',
                      cursor: 'pointer',
                      fontSize: '12px',
                      fontWeight: 600,
                    }}
                  >
                    + Add Component Row
                  </button>

                  <button
                    type="submit"
                    disabled={loading}
                    style={{
                      padding: '7px 20px',
                      backgroundColor: '#004d40',
                      color: '#fff',
                      border: 'none',
                      borderRadius: '4px',
                      cursor: 'pointer',
                      fontSize: '13px',
                      fontWeight: 600,
                    }}
                  >
                    {loading ? 'Saving...' : 'Save Recipe'}
                  </button>
                </div>
              </form>
            </div>
          )}

          {/* BOM List */}
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))',
              gap: '16px',
            }}
          >
            {boms.map((b) => (
              <div
                key={b.id}
                style={{
                  backgroundColor: '#fff',
                  border: '1px solid #ddd',
                  borderRadius: '6px',
                  padding: '16px',
                  boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'flex-start',
                    borderBottom: '1px solid #eee',
                    paddingBottom: '8px',
                    marginBottom: '10px',
                  }}
                >
                  <div>
                    <h3
                      style={{
                        margin: 0,
                        fontSize: '15px',
                        color: '#004d40',
                        fontWeight: 'bold',
                      }}
                    >
                      {b.bom_name}
                    </h3>
                    <div
                      style={{
                        fontSize: '12px',
                        color: '#555',
                        marginTop: '2px',
                      }}
                    >
                      Output:{' '}
                      <strong style={{ color: '#00251a' }}>
                        {b.finished_stock_item_name}
                      </strong>{' '}
                      ({b.base_output_qty} units)
                    </div>
                  </div>
                  <span
                    style={{
                      padding: '2px 6px',
                      backgroundColor: b.is_active ? '#e8f5e9' : '#ffebee',
                      color: b.is_active ? '#2e7d32' : '#c62828',
                      borderRadius: '3px',
                      fontSize: '11px',
                      fontWeight: 600,
                    }}
                  >
                    {b.is_active ? 'Active' : 'Inactive'}
                  </span>
                </div>

                <div style={{ fontSize: '12px', color: '#666', marginBottom: '8px' }}>
                  Components ({b.components.length}):
                </div>
                <ul
                  style={{
                    margin: '0 0 14px 0',
                    paddingLeft: '18px',
                    fontSize: '12px',
                    color: '#333',
                  }}
                >
                  {b.components.map((comp) => (
                    <li key={comp.id} style={{ marginBottom: '3px' }}>
                      <strong>{comp.raw_stock_item_name}</strong>: {comp.required_qty} units
                      {comp.source_godown_name && (
                        <span style={{ color: '#888' }}>
                          {' '}
                          (from {comp.source_godown_name})
                        </span>
                      )}
                    </li>
                  ))}
                </ul>

                <button
                  onClick={() => {
                    setSelectedBomId(b.id);
                    setActiveTab('produce');
                  }}
                  style={{
                    width: '100%',
                    padding: '6px',
                    backgroundColor: '#e0f2f1',
                    color: '#004d40',
                    border: '1px solid #b2dfdb',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontSize: '12px',
                    fontWeight: 600,
                  }}
                >
                  Produce with this Recipe →
                </button>
              </div>
            ))}
            {boms.length === 0 && (
              <div
                style={{
                  gridColumn: '1 / -1',
                  textAlign: 'center',
                  padding: '30px',
                  color: '#888',
                  backgroundColor: '#fff',
                  borderRadius: '6px',
                  border: '1px dashed #ccc',
                }}
              >
                No Bill of Materials recipes defined yet. Click "+ Define New BOM Recipe" to create your first production recipe.
              </div>
            )}
          </div>
        </div>
      )}

      {/* TAB 2: MANUFACTURING STOCK JOURNAL */}
      {activeTab === 'produce' && (
        <div>
          <div
            style={{
              backgroundColor: '#fff',
              border: '1px solid #ddd',
              borderRadius: '6px',
              padding: '18px',
              marginBottom: '20px',
              boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
            }}
          >
            <h2
              style={{
                margin: '0 0 16px 0',
                fontSize: '16px',
                color: '#004d40',
                fontWeight: 'bold',
              }}
            >
              Manufacturing Journal Entry
            </h2>

            <form onSubmit={handleExecuteProduction}>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
                  gap: '14px',
                  marginBottom: '16px',
                }}
              >
                <div>
                  <label
                    style={{
                      display: 'block',
                      fontSize: '12px',
                      fontWeight: 600,
                      marginBottom: '4px',
                    }}
                  >
                    Select BOM Recipe *
                  </label>
                  <select
                    value={selectedBomId}
                    onChange={(e) => setSelectedBomId(e.target.value)}
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '13px',
                      border: '1px solid #ccc',
                      borderRadius: '4px',
                    }}
                  >
                    <option value="">-- Select Recipe --</option>
                    {boms.map((b) => (
                      <option key={b.id} value={b.id}>
                        {b.bom_name} ({b.finished_stock_item_name})
                      </option>
                    ))}
                  </select>
                </div>

                <div>
                  <label
                    style={{
                      display: 'block',
                      fontSize: '12px',
                      fontWeight: 600,
                      marginBottom: '4px',
                    }}
                  >
                    Production Date *
                  </label>
                  <input
                    type="date"
                    value={productionDate}
                    onChange={(e) => setProductionDate(e.target.value)}
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '13px',
                      border: '1px solid #ccc',
                      borderRadius: '4px',
                      boxSizing: 'border-box',
                    }}
                  />
                </div>

                <div>
                  <label
                    style={{
                      display: 'block',
                      fontSize: '12px',
                      fontWeight: 600,
                      marginBottom: '4px',
                    }}
                  >
                    Produced Quantity *
                  </label>
                  <input
                    type="text"
                    value={producedQty}
                    onChange={(e) => setProducedQty(e.target.value)}
                    placeholder="e.g. 10"
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '13px',
                      border: '1px solid #ccc',
                      borderRadius: '4px',
                      boxSizing: 'border-box',
                    }}
                  />
                </div>

                <div>
                  <label
                    style={{
                      display: 'block',
                      fontSize: '12px',
                      fontWeight: 600,
                      marginBottom: '4px',
                    }}
                  >
                    Destination Godown *
                  </label>
                  <select
                    value={destGodownId}
                    onChange={(e) => setDestGodownId(e.target.value)}
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '13px',
                      border: '1px solid #ccc',
                      borderRadius: '4px',
                    }}
                  >
                    <option value="">-- Select Destination Godown --</option>
                    {godowns.map((g) => (
                      <option key={g.id} value={g.id}>
                        {g.name}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Live Scaling Component Preview */}
              {currentBom && (
                <div
                  style={{
                    backgroundColor: '#fafafa',
                    border: '1px solid #e0e0e0',
                    borderRadius: '4px',
                    padding: '14px',
                    marginBottom: '16px',
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      marginBottom: '8px',
                    }}
                  >
                    <h4
                      style={{
                        margin: 0,
                        fontSize: '13px',
                        color: '#004d40',
                        fontWeight: 600,
                      }}
                    >
                      Proportional Component Consumption Preview (Scaling Factor: {scaleRatio}x)
                    </h4>
                    <span style={{ fontSize: '12px', color: '#666' }}>
                      Base Output: {currentBom.base_output_qty} units → Target: {producedQty} units
                    </span>
                  </div>

                  <table
                    style={{
                      width: '100%',
                      borderCollapse: 'collapse',
                      fontSize: '12px',
                    }}
                  >
                    <thead>
                      <tr
                        style={{
                          backgroundColor: '#f0f0f0',
                          borderBottom: '1px solid #ccc',
                          textAlign: 'left',
                        }}
                      >
                        <th style={{ padding: '6px 8px' }}>Component Item</th>
                        <th style={{ padding: '6px 8px' }}>Source Godown</th>
                        <th style={{ padding: '6px 8px' }}>Base Required</th>
                        <th
                          style={{
                            padding: '6px 8px',
                            fontWeight: 'bold',
                            color: '#004d40',
                          }}
                        >
                          Scaled Quantity to Consume
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {currentBom.components.map((comp) => {
                        const scaledQty = previewFactoryConsumption(producedQty, comp.required_qty, currentBom.base_output_qty);
                        return (
                          <tr
                            key={comp.id}
                            style={{ borderBottom: '1px solid #eee' }}
                          >
                            <td style={{ padding: '6px 8px', fontWeight: 600 }}>
                              {comp.raw_stock_item_name}
                            </td>
                            <td style={{ padding: '6px 8px', color: '#666' }}>
                              {comp.source_godown_name || 'Destination Godown'}
                            </td>
                            <td style={{ padding: '6px 8px' }}>
                              {comp.required_qty}
                            </td>
                            <td
                              style={{
                                padding: '6px 8px',
                                fontWeight: 'bold',
                                color: '#004d40',
                              }}
                            >
                              {scaledQty}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}

              {/* Additional Overheads */}
              <div style={{ marginBottom: '16px' }}>
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    marginBottom: '8px',
                  }}
                >
                  <h4
                    style={{
                      margin: 0,
                      fontSize: '13px',
                      color: '#333',
                      fontWeight: 600,
                    }}
                  >
                    Additional Overhead Expenses (Wages, Power, Packing)
                  </h4>
                  <button
                    type="button"
                    onClick={addOverheadRow}
                    style={{
                      padding: '4px 10px',
                      backgroundColor: '#e0f2f1',
                      color: '#004d40',
                      border: '1px solid #b2dfdb',
                      borderRadius: '3px',
                      cursor: 'pointer',
                      fontSize: '12px',
                    }}
                  >
                    + Add Overhead
                  </button>
                </div>

                {overheads.length > 0 && (
                  <table
                    style={{
                      width: '100%',
                      borderCollapse: 'collapse',
                      fontSize: '12px',
                      marginBottom: '10px',
                    }}
                  >
                    <thead>
                      <tr
                        style={{
                          backgroundColor: '#f5f5f5',
                          borderBottom: '1px solid #ddd',
                          textAlign: 'left',
                        }}
                      >
                        <th style={{ padding: '6px 8px' }}>Expense Ledger</th>
                        <th style={{ padding: '6px 8px' }}>Amount (₹)</th>
                        <th style={{ padding: '6px 8px' }}>Description</th>
                        <th style={{ padding: '6px 8px', textAlign: 'right' }}>
                          Action
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {overheads.map((ovh) => (
                        <tr
                          key={ovh.id}
                          style={{ borderBottom: '1px solid #eee' }}
                        >
                          <td style={{ padding: '6px 8px' }}>
                            <select
                              value={ovh.overhead_ledger_id}
                              onChange={(e) =>
                                updateOverheadRow(
                                  ovh.id,
                                  'overhead_ledger_id',
                                  e.target.value
                                )
                              }
                              style={{
                                width: '100%',
                                padding: '4px 8px',
                                fontSize: '12px',
                                border: '1px solid #ccc',
                                borderRadius: '3px',
                              }}
                            >
                              <option value="">-- Select Expense Ledger --</option>
                              {ledgers.map((l) => (
                                <option key={l.id} value={l.id}>
                                  {l.name}
                                </option>
                              ))}
                            </select>
                          </td>
                          <td style={{ padding: '6px 8px' }}>
                            <input
                              type="text"
                              value={ovh.amount}
                              onChange={(e) =>
                                updateOverheadRow(
                                  ovh.id,
                                  'amount',
                                  e.target.value
                                )
                              }
                              style={{
                                width: '100px',
                                padding: '4px 8px',
                                fontSize: '12px',
                                border: '1px solid #ccc',
                                borderRadius: '3px',
                              }}
                            />
                          </td>
                          <td style={{ padding: '6px 8px' }}>
                            <input
                              type="text"
                              value={ovh.description}
                              onChange={(e) =>
                                updateOverheadRow(
                                  ovh.id,
                                  'description',
                                  e.target.value
                                )
                              }
                              placeholder="e.g. Machine Operator Wages"
                              style={{
                                width: '100%',
                                padding: '4px 8px',
                                fontSize: '12px',
                                border: '1px solid #ccc',
                                borderRadius: '3px',
                                boxSizing: 'border-box',
                              }}
                            />
                          </td>
                          <td
                            style={{ padding: '6px 8px', textAlign: 'right' }}
                          >
                            <button
                              type="button"
                              onClick={() => removeOverheadRow(ovh.id)}
                              style={{
                                padding: '2px 6px',
                                backgroundColor: '#ffebee',
                                color: '#c62828',
                                border: '1px solid #ffcdd2',
                                borderRadius: '3px',
                                cursor: 'pointer',
                                fontSize: '11px',
                              }}
                            >
                              ✕
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>

              {/* Batch & Narration */}
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: '1fr 2fr',
                  gap: '14px',
                  marginBottom: '16px',
                }}
              >
                <div>
                  <label
                    style={{
                      display: 'block',
                      fontSize: '12px',
                      fontWeight: 600,
                      marginBottom: '4px',
                    }}
                  >
                    Batch Number (Optional)
                  </label>
                  <input
                    type="text"
                    value={batchNumber}
                    onChange={(e) => setBatchNumber(e.target.value)}
                    placeholder="e.g. BATCH-2026-05"
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '13px',
                      border: '1px solid #ccc',
                      borderRadius: '4px',
                      boxSizing: 'border-box',
                    }}
                  />
                </div>
                <div>
                  <label
                    style={{
                      display: 'block',
                      fontSize: '12px',
                      fontWeight: 600,
                      marginBottom: '4px',
                    }}
                  >
                    Narration
                  </label>
                  <input
                    type="text"
                    value={narration}
                    onChange={(e) => setNarration(e.target.value)}
                    placeholder="Manufacturing Stock Journal Narration"
                    style={{
                      width: '100%',
                      padding: '6px 10px',
                      fontSize: '13px',
                      border: '1px solid #ccc',
                      borderRadius: '4px',
                      boxSizing: 'border-box',
                    }}
                  />
                </div>
              </div>

              <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                <button
                  type="submit"
                  disabled={loading}
                  style={{
                    padding: '8px 24px',
                    backgroundColor: '#004d40',
                    color: '#fff',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontSize: '14px',
                    fontWeight: 600,
                  }}
                >
                  {loading ? 'Processing...' : 'Post Manufacturing Journal (Alt+P)'}
                </button>
              </div>
            </form>
          </div>

          {/* Production Result Summary Card */}
          {prodResult && (
            <div
              style={{
                backgroundColor: '#fff',
                border: '1px solid #2e7d32',
                borderRadius: '6px',
                padding: '18px',
                boxShadow: '0 2px 6px rgba(46,125,50,0.1)',
              }}
            >
              <h3
                style={{
                  margin: '0 0 12px 0',
                  fontSize: '16px',
                  color: '#2e7d32',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                }}
              >
                <span>✓</span> Production Execution Summary
              </h3>

              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
                  gap: '12px',
                  marginBottom: '16px',
                }}
              >
                <div
                  style={{
                    padding: '10px',
                    backgroundColor: '#f1f8e9',
                    borderRadius: '4px',
                  }}
                >
                  <div style={{ fontSize: '11px', color: '#555' }}>
                    Produced Quantity
                  </div>
                  <div
                    style={{
                      fontSize: '16px',
                      fontWeight: 'bold',
                      color: '#1b5e20',
                    }}
                  >
                    {prodResult.produced_qty} units
                  </div>
                </div>

                <div
                  style={{
                    padding: '10px',
                    backgroundColor: '#f1f8e9',
                    borderRadius: '4px',
                  }}
                >
                  <div style={{ fontSize: '11px', color: '#555' }}>
                    Total Raw Material Cost
                  </div>
                  <div
                    style={{
                      fontSize: '16px',
                      fontWeight: 'bold',
                      color: '#1b5e20',
                    }}
                  >
                    ₹{prodResult.total_raw_material_cost}
                  </div>
                </div>

                <div
                  style={{
                    padding: '10px',
                    backgroundColor: '#f1f8e9',
                    borderRadius: '4px',
                  }}
                >
                  <div style={{ fontSize: '11px', color: '#555' }}>
                    Total Overhead Cost
                  </div>
                  <div
                    style={{
                      fontSize: '16px',
                      fontWeight: 'bold',
                      color: '#1b5e20',
                    }}
                  >
                    ₹{prodResult.total_overhead_cost}
                  </div>
                </div>

                <div
                  style={{
                    padding: '10px',
                    backgroundColor: '#e8f5e9',
                    borderRadius: '4px',
                    border: '1px solid #c8e6c9',
                  }}
                >
                  <div style={{ fontSize: '11px', color: '#555' }}>
                    Landed Unit Cost
                  </div>
                  <div
                    style={{
                      fontSize: '18px',
                      fontWeight: 'bold',
                      color: '#004d40',
                    }}
                  >
                    ₹{prodResult.landed_unit_cost}
                  </div>
                </div>
              </div>

              <h4
                style={{
                  margin: '0 0 8px 0',
                  fontSize: '13px',
                  color: '#333',
                }}
              >
                Raw Material FIFO Consumption Breakdown
              </h4>
              <table
                style={{
                  width: '100%',
                  borderCollapse: 'collapse',
                  fontSize: '12px',
                }}
              >
                <thead>
                  <tr
                    style={{
                      backgroundColor: '#f5f5f5',
                      borderBottom: '1px solid #ddd',
                      textAlign: 'left',
                    }}
                  >
                    <th style={{ padding: '6px 8px' }}>Component Item</th>
                    <th style={{ padding: '6px 8px' }}>Consumed Quantity</th>
                    <th style={{ padding: '6px 8px', textAlign: 'right' }}>
                      Consumed Cost (₹)
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {prodResult.consumed_components.map((c, i) => (
                    <tr key={i} style={{ borderBottom: '1px solid #eee' }}>
                      <td style={{ padding: '6px 8px', fontWeight: 600 }}>
                        {c.raw_stock_item_name}
                      </td>
                      <td style={{ padding: '6px 8px' }}>{c.consumed_qty}</td>
                      <td style={{ padding: '6px 8px', textAlign: 'right' }}>
                        ₹{c.consumed_cost}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
