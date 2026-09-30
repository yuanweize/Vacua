#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Vacua — Apple Intelligence Readiness & Environment Qualification Probe
# ==============================================================================
# Truthfully reports host hardware, operating system, developer toolchain,
# FoundationModels compilation capability, and Apple SystemLanguageModel
# runtime availability.
#
# NEVER fakes hardware eligibility or runtime verification.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

JSON_MODE=false
for arg in "$@"; do
  if [ "$arg" == "--json" ]; then
    JSON_MODE=true
  fi
done

# 1. Host Architecture & Kernel
ARCH="$(uname -m)"

# 2. Operating System Details
OS_NAME="$(sw_vers -productName 2>/dev/null || echo "macOS")"
OS_VERSION="$(sw_vers -productVersion 2>/dev/null || echo "0.0")"
OS_BUILD="$(sw_vers -buildVersion 2>/dev/null || echo "unknown")"

# 3. Xcode Toolchain
XCODE_SELECT_PATH="$(xcode-select -p 2>/dev/null || echo "none")"
XCODE_VERSION="$(xcodebuild -version 2>/dev/null | tr '\n' ' ' | sed 's/  */ /g' || echo "unavailable")"

# 4. FoundationModels Module Compilation Availability
SDK_AVAILABLE=false
COMPILE_PROOF_OUTPUT=""
if swift run --package-path "${REPO_ROOT}/apple/VacuaIntelligence" foundation-models-proof >/dev/null 2>&1; then
  SDK_AVAILABLE=true
  COMPILE_PROOF_OUTPUT="PASSED (FoundationModels framework imported and compiled successfully)"
else
  COMPILE_PROOF_OUTPUT="FAILED (FoundationModels framework unavailable or compilation failed)"
fi

# 5. OS Support Level (vacua-intelligence requires macOS 26.0+)
OS_SUPPORTED=false
OS_MAJOR=$(echo "${OS_VERSION}" | cut -d'.' -f1)
if [ "${OS_MAJOR}" -ge 26 ] 2>/dev/null; then
  OS_SUPPORTED=true
fi

# 6. Real Runtime SystemLanguageModel Availability via VacuaIntelligence
INTEL_CLI=""
if command -v vacua-intelligence >/dev/null 2>&1; then
  INTEL_CLI="vacua-intelligence"
elif [ -f "${REPO_ROOT}/apple/VacuaIntelligence/.build/debug/vacua-intelligence" ]; then
  INTEL_CLI="${REPO_ROOT}/apple/VacuaIntelligence/.build/debug/vacua-intelligence"
fi

DEVICE_ELIGIBLE=false
APPLE_INTELLIGENCE_ENABLED=false
MODEL_READY=false
MODEL_AVAILABLE=false
RAW_AVAILABILITY="unknown"
FALLBACK_REASON=""
PROVIDER_USED="deterministic-fallback"

if [ -n "${INTEL_CLI}" ]; then
  STATUS_JSON=$("${INTEL_CLI}" status 2>/dev/null || echo "{}")
  RAW_AVAILABILITY=$(echo "${STATUS_JSON}" | python3 -c 'import sys, json; print(json.load(sys.stdin).get("apple_model_availability", "unknown"))' 2>/dev/null || echo "unknown")
  FALLBACK_REASON=$(echo "${STATUS_JSON}" | python3 -c 'import sys, json; print(json.load(sys.stdin).get("fallback_reason", "none"))' 2>/dev/null || echo "none")
  PROVIDER_USED=$(echo "${STATUS_JSON}" | python3 -c 'import sys, json; print(json.load(sys.stdin).get("provider_used", "deterministic-fallback"))' 2>/dev/null || echo "deterministic-fallback")

  case "${RAW_AVAILABILITY}" in
    "available")
      DEVICE_ELIGIBLE=true
      APPLE_INTELLIGENCE_ENABLED=true
      MODEL_READY=true
      MODEL_AVAILABLE=true
      ;;
    "deviceNotEligible")
      DEVICE_ELIGIBLE=false
      APPLE_INTELLIGENCE_ENABLED=false
      MODEL_READY=false
      MODEL_AVAILABLE=false
      ;;
    "appleIntelligenceNotEnabled")
      DEVICE_ELIGIBLE=true
      APPLE_INTELLIGENCE_ENABLED=false
      MODEL_READY=false
      MODEL_AVAILABLE=false
      ;;
    "modelNotReady")
      DEVICE_ELIGIBLE=true
      APPLE_INTELLIGENCE_ENABLED=true
      MODEL_READY=false
      MODEL_AVAILABLE=false
      ;;
    *)
      DEVICE_ELIGIBLE=false
      APPLE_INTELLIGENCE_ENABLED=false
      MODEL_READY=false
      MODEL_AVAILABLE=false
      ;;
  esac
fi

# 7. Compute Granular Overall State
QUALIFICATION_STATE="UNKNOWN"
if [ "${MODEL_AVAILABLE}" = true ] && [ "${PROVIDER_USED}" = "apple-system" ]; then
  QUALIFICATION_STATE="RUNTIME_VERIFIED"
elif [ "${SDK_AVAILABLE}" = true ]; then
  QUALIFICATION_STATE="IMPLEMENTED / Awaiting eligible-hardware runtime qualification"
else
  QUALIFICATION_STATE="UNSUPPORTED_HOST"
fi

# 8. Render Output
if [ "${JSON_MODE}" = true ]; then
  cat <<EOF
{
  "schema": "vacua.apple-intelligence.readiness.v1",
  "system_info": {
    "architecture": "${ARCH}",
    "os_name": "${OS_NAME}",
    "os_version": "${OS_VERSION}",
    "os_build": "${OS_BUILD}",
    "xcode_version": "${XCODE_VERSION}",
    "developer_dir": "${XCODE_SELECT_PATH}"
  },
  "readiness_matrix": {
    "SDK_AVAILABLE": ${SDK_AVAILABLE},
    "OS_SUPPORTED": ${OS_SUPPORTED},
    "DEVICE_ELIGIBLE": ${DEVICE_ELIGIBLE},
    "APPLE_INTELLIGENCE_ENABLED": ${APPLE_INTELLIGENCE_ENABLED},
    "MODEL_READY": ${MODEL_READY},
    "MODEL_AVAILABLE": ${MODEL_AVAILABLE}
  },
  "runtime_probe": {
    "raw_availability": "${RAW_AVAILABILITY}",
    "fallback_reason": "${FALLBACK_REASON}",
    "provider_used": "${PROVIDER_USED}",
    "compile_proof": "${COMPILE_PROOF_OUTPUT}"
  },
  "qualification_state": "${QUALIFICATION_STATE}"
}
EOF
else
  echo "================================================================================"
  echo "Vacua — Apple Intelligence Readiness Probe"
  echo "================================================================================"
  echo "Host System:"
  echo "  - Architecture:              ${ARCH}"
  echo "  - Operating System:          ${OS_NAME} ${OS_VERSION} (${OS_BUILD})"
  echo "  - Xcode Toolchain:           ${XCODE_VERSION}"
  echo "  - Developer Directory:       ${XCODE_SELECT_PATH}"
  echo ""
  echo "Framework & Compile Readiness:"
  echo "  - FoundationModels Module:   ${SDK_AVAILABLE} (${COMPILE_PROOF_OUTPUT})"
  echo "  - OS Level Supported (>=26): ${OS_SUPPORTED}"
  echo ""
  echo "Apple SystemLanguageModel Runtime Availability:"
  echo "  - [SDK_AVAILABLE]:             ${SDK_AVAILABLE}"
  echo "  - [OS_SUPPORTED]:              ${OS_SUPPORTED}"
  echo "  - [DEVICE_ELIGIBLE]:           ${DEVICE_ELIGIBLE}"
  echo "  - [APPLE_INTELLIGENCE_ENABLED]: ${APPLE_INTELLIGENCE_ENABLED}"
  echo "  - [MODEL_READY]:               ${MODEL_READY}"
  echo "  - [MODEL_AVAILABLE]:           ${MODEL_AVAILABLE}"
  echo ""
  echo "Runtime Status Detail:"
  echo "  - API Availability Enum:     ${RAW_AVAILABILITY}"
  echo "  - Fallback Reason:           ${FALLBACK_REASON}"
  echo "  - Active Intelligence Engine: ${PROVIDER_USED}"
  echo ""
  echo "Overall Qualification State:"
  echo "  >>> ${QUALIFICATION_STATE} <<<"
  echo "================================================================================"
fi
