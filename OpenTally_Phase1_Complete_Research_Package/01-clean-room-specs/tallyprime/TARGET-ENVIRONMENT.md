# OpenTally Target Environment Identification & Assessment

## 1. Target Environment Identification Summary

In accordance with Section 2 of the Phase 1 Mandate, this document formally records the research target environment, product specifications, runtime availability, and test constraints.

| Parameter | Specification / Assessment Record | Provenance / Status |
| :--- | :--- | :--- |
| **Target Product Name** | **TallyPrime** & **TallyPrime Edit Log** | Official Software Releases by Tally Solutions Pvt. Ltd. |
| **Target Release & Version** | **Release 4.1** (Primary Baseline), **Release 3.0.1 / 2.1** (Edit Log Baseline) | Official Production Releases (2022–2024) |
| **Target Architecture / Build** | 64-bit Windows Native Architecture (Tally.exe) | Official Documentation Baseline |
| **Operating System** | Windows 10/11 64-bit, Windows Server 2016/2019/2022 | Official Hardware/OS Requirements |
| **Host Environment State** | Linux Container (Ubuntu / Debian x86_64 Sandbox) | Inspected at runtime via `ps aux`, `ss -tulpn` |
| **Target Runtime Presence** | **NOT RUNNING** (No native Tally.exe process active) | Verified via process scan (`ps aux`) |
| **Integration Port (9000)** | **INACTIVE** (Socket `0.0.0.0:9000` closed/unbound) | Verified via socket check (`ss -tulpn`) |
| **Synthetic Test Company** | Desk Research Baseline: "National Synthetic Trading Co." | Designated Synthetic Company Profile |
| **Research Methodology** | **Systematic Desk Research & Documented Behavioral Analysis** | Strictly compliant with Clean-Room Rules |

## 2. Environment Verification Findings

A comprehensive environment inspection was executed prior to functional research:
1. **Process Inspection**: Running processes were scanned (`ps aux`). No Windows emulation (Wine) or native Tally daemon was detected.
2. **Port Audit**: Network interfaces were inspected (`ss -tulpn`). Port 9000 (TallyPrime default HTTP XML listener) was not bound.
3. **Operational Constraint Enforcement**: Because no live TallyPrime instance is currently hosted inside this Linux container, **all behavioral findings must be derived from official vendor documentation, developer integration guides, and verified accounting governance**.
4. **Classification Rule**: No assertion derived from desk research shall be reported as "DIRECT_OBSERVATION". All research findings are strictly classified as `OFFICIAL_DOCUMENTATION`, `INFERENCE`, `HYPOTHESIS`, or `UNKNOWN`.

## 3. Designated Synthetic Test Company Profile
For all test specifications and XML payloads, the following synthetic company parameters are established:
- **Company Name**: National Synthetic Trade & Manufacturing Enterprises
- **Mailing Name**: National Synthetic Trade & Manufacturing Enterprises
- **State**: Karnataka (State Code: 29)
- **Country**: India
- **Base Currency**: INR (₹)
- **Financial Year Begins**: 01-Apr-2024
- **Books Beginning From**: 01-Apr-2024
- **GSTIN**: `29AAACN1234F1Z5` (Synthetic Karnataka GSTIN)
- **PAN**: `AAACN1234F`
- **Registration Type**: Regular
- **Default Godown**: Main Location
