import { Company } from '../types';
import { SelectCompanyView } from './company/SelectCompanyView';

interface CompanySelectModalProps {
  isOpen: boolean;
  activeCompanyId: string | null;
  onClose: () => void;
  onCompanySelected: (company: Company) => void;
  onCreateNew: () => void;
  onImportXml?: () => void;
  onRestoreBackup?: () => void;
}

export function CompanySelectModal({
  isOpen,
  activeCompanyId,
  onClose,
  onCompanySelected,
  onCreateNew,
  onImportXml,
  onRestoreBackup,
}: CompanySelectModalProps) {
  if (!isOpen) return null;

  return (
    <div className="ot-modal-backdrop" onClick={onClose} data-testid="company-select-modal">
      <div className="ot-modal-content ot-modal-lg" onClick={(e) => e.stopPropagation()}>
        <SelectCompanyView
          activeCompanyId={activeCompanyId}
          onCompanySelected={(company) => {
            onCompanySelected(company);
            onClose();
          }}
          onCreateCompany={onCreateNew}
          onImportXml={() => {
            onClose();
            if (onImportXml) onImportXml();
          }}
          onRestoreBackup={() => {
            onClose();
            if (onRestoreBackup) onRestoreBackup();
          }}
          onClose={onClose}
        />
      </div>
    </div>
  );
}


