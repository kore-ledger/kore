#!/bin/bash

# ===================== Configuración =====================
# Runner por defecto. Para nextest: export RUNNER="cargo nextest run"
RUNNER="${RUNNER:-cargo}"

# Flags comunes para los tests (p.ej. --all-targets, --doc)
BASE_FLAGS="${BASE_FLAGS:---all-targets}"

# Flags de instalación de cargo-hack
INSTALL_FLAGS="${INSTALL_FLAGS:---locked}"

# Directorio de logs de cada ejecución
LOG_DIR="${LOG_DIR:-target/test-logs}"

# Matriz de combinaciones por paquete:
# Formato por línea: <paquete>|<combo1>;<combo2>;...
# - Cada combo son flags añadidos después de `-p <paquete>`
# - Deja vacío tras el '|' si no quieres combos extra para ese paquete
read -r -d '' MATRIX <<'EOF'
identity|--all-features
tell|--all-features
network|--all-features
kore-base|--no-default-features --features sqlite,ext-sqlite;--no-default-features --features rocksdb,ext-sqlite
kore-bridge|--no-default-features --features sqlite,ext-sqlite;--no-default-features --features rocksdb,ext-sqlite
kore-http|--no-default-features --features sqlite,ext-sqlite;--no-default-features --features rocksdb,ext-sqlite
EOF
# =========================================================

# ======== Estado global para el resumen =========
SUCCESSES=()           # etiquetas OK
FAIL_LABELS=()         # etiquetas fallidas
FAIL_DETAILS=()        # detalles por índice (resumen + lista de tests)
EXIT_CODE=0

# ======== Colores / banner (desactivados si NO_COLOR) ========
if [ -n "${NO_COLOR:-}" ]; then
  BOLD=""; DIM=""; RED=""; GREEN=""; CYAN=""; YELLOW=""; RESET=""
else
  BOLD=$'\033[1m'
  DIM=$'\033[2m'
  RED=$'\033[31m'
  GREEN=$'\033[32m'
  YELLOW=$'\033[33m'
  CYAN=$'\033[36m'
  RESET=$'\033[0m'
fi

banner() {
  # $1 = título principal
  # $2 = subtítulo
  local title="$1"
  local subtitle="$2"
  local line="======================================================================="
  echo
  echo "${CYAN}${line}${RESET}"
  printf "${BOLD}▶ %s${RESET}\n" "$title"
  if [ -n "$subtitle" ]; then
    printf "${DIM}   %s${RESET}\n" "$subtitle"
  fi
  echo "${CYAN}${line}${RESET}"
}

# ======== Utilidades ========
ensure_tools() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: 'cargo' no está en PATH." >&2
    exit 1
  fi
  if ! cargo hack --version >/dev/null 2>&1; then
    echo "Instalando cargo-hack..."
    cargo install cargo-hack ${INSTALL_FLAGS}
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo hack --version >/dev/null 2>&1 || {
      echo "No pude invocar 'cargo hack' tras la instalación. Revisa tu PATH." >&2
      exit 2
    }
  else
    echo "cargo-hack OK: $(cargo hack --version)"
  fi
  mkdir -p "${LOG_DIR}"
}

# Normaliza espacios en un combo para usarlo como clave
normalize_combo() {
  echo "$1" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e 's/[[:space:]]\{1,\}/ /g'
}

# Extrae lista de tests fallidos desde el log (formato libtest)
extract_failures() {
  # $1 = ruta del log
  awk '
    /^failures:\s*$/ { in=1; next }
    in && NF==0 { in=0 }
    in { sub(/^[[:space:]]+/, "", $0); print $0 }
  ' "$1"
}

# Ejecuta un comando de test, muestra banner, guarda log y clasifica resultado
run_step() {
  # $1 = label
  # $2 = subtitle
  shift 0
  local label="$1"; shift
  local subtitle="$1"; shift

  banner "$label" "$subtitle"

  local safe
  safe="$(echo "${label} ${subtitle}" | tr ' /[]:,|' '________')"
  local logfile="${LOG_DIR}/$(date +%Y%m%d_%H%M%S)_${safe}.log"

  if "$@" > >(tee "${logfile}") 2>&1; then
    echo "${GREEN}✔ OK${RESET}  (${logfile})"
    SUCCESSES+=("${label} | ${subtitle}")
  else
    local rc=$?
    echo "${RED}✖ FALLÓ (rc=${rc})${RESET}  (${logfile})"

    local summary
    summary="$(grep -E 'test result:' "${logfile}" | tail -1 || true)"

    local failed
    failed="$(extract_failures "${logfile}")"

    local details=""
    if [ -n "${summary}" ]; then details+="${summary}"$'\n'; fi
    if [ -n "${failed}" ]; then details+="failed tests:"$'\n'"${failed}"; fi
    if [ -z "${details}" ]; then details="(no pude extraer nombres de tests; mira el log)"; fi

    FAIL_LABELS+=("${label} | ${subtitle}")
    FAIL_DETAILS+=("${details}")
    EXIT_CODE=1
  fi
}

# Filtra el MATRIX si pasas nombres de paquetes como argumentos
filter_matrix_lines() {
  if [[ "$#" -eq 0 ]]; then
    echo "${MATRIX}"
    return
  fi
  local filtered=""
  while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    local pkg="${line%%|*}"
    local want
    for want in "$@"; do
      if [[ "${pkg}" == "${want}" ]]; then
        filtered+="${line}"$'\n'
        break
      fi
    done
  done <<< "${MATRIX}"
  echo "${filtered}"
}

# Agrupa paquetes por combo (bash 3.2: arrays paralelas)
GROUP_KEYS=()  # combos normalizados
GROUP_PKGS=()  # lista de pkgs (separados por espacio)

add_to_group() {
  # $1 = combo (normalizado)
  # $2 = pkg
  local key="$1"
  local pkg="$2"
  local i
  for ((i=0; i<${#GROUP_KEYS[@]}; i++)); do
    if [[ "${GROUP_KEYS[$i]}" == "${key}" ]]; then
      case " ${GROUP_PKGS[$i]} " in
        *" ${pkg} "*) : ;;
        *) GROUP_PKGS[$i]="${GROUP_PKGS[$i]} ${pkg}" ;;
      esac
      return
    fi
  done
  GROUP_KEYS+=("${key}")
  GROUP_PKGS+=("${pkg}")
}

# Construye grupos a partir del MATRIX (y filtro opcional de paquetes)
build_groups() {
  local lines
  lines="$(filter_matrix_lines "$@")"
  while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    local pkg combos_str
    pkg="${line%%|*}"
    combos_str="${line#*|}"

    IFS=';' read -r -a combos <<< "${combos_str}"
    local combo
    for combo in "${combos[@]}"; do
      combo="$(echo "${combo}" | xargs)"
      [[ -z "${combo}" ]] && continue
      local key
      key="$(normalize_combo "${combo}")"
      add_to_group "${key}" "${pkg}"
    done
  done <<< "${lines}"
}

# Precompila (una vez por grupo) y luego ejecuta tests por paquete (informe individual)
run_groups() {
  local i
  for ((i=0; i<${#GROUP_KEYS[@]}; i++)); do
    local key="${GROUP_KEYS[$i]}"
    local pkgs="${GROUP_PKGS[$i]}"

    # ===== Precompilación única del grupo =====
    # Construimos comando para compilar tests sin ejecutarlos
    local pre_label="precompile"
    local pre_sub="pkgs: $(echo "${pkgs}" | tr ' ' ',')  | combo: ${key}  ${BASE_FLAGS}  (--no-run)"
    local pre_cmd=()

    # Usamos cargo hack para pasar varios -p a la vez
    pre_cmd=(cargo hack test)
    local p
    for p in ${pkgs}; do pre_cmd+=(-p "$p"); done
    # shellcheck disable=SC2206
    local key_arr=( $key )
    # shellcheck disable=SC2206
    local base_arr=( $BASE_FLAGS )
    pre_cmd+=("${key_arr[@]}")
    if [[ -n "${BASE_FLAGS}" ]]; then pre_cmd+=("${base_arr[@]}"); fi
    pre_cmd+=(--no-run)

    run_step "${pre_label}" "${pre_sub}" "${pre_cmd[@]}"

    # ===== Ejecución individual por paquete (informe por paquete) =====
    for p in ${pkgs}; do
      local label="${p}"
      local subtitle="[${key}] ${BASE_FLAGS}"
      local cmd=()

      # Elegimos runner para la ejecución (no para precompilación)
      if [[ "${RUNNER}" == "cargo" ]]; then
        cmd=(cargo test -p "${p}")
      else
        # RUNNER="cargo nextest run" u otro
        # shellcheck disable=SC2206
        cmd=( ${RUNNER} -p "${p}" )
      fi

      cmd+=("${key_arr[@]}")
      if [[ -n "${BASE_FLAGS}" ]]; then cmd+=("${base_arr[@]}"); fi

      run_step "${label}" "${subtitle}" "${cmd[@]}"
    done
  done
}

print_summary() {
  echo
  echo "${BOLD}==================== RESUMEN ====================${RESET}"
  if [ "${#SUCCESSES[@]}" -gt 0 ]; then
    echo "${GREEN}✔ OK (${#SUCCESSES[@]}):${RESET}"
    local s
    for s in "${SUCCESSES[@]}"; do
      echo "  - ${s}"
    done
  fi
  if [ "${#FAIL_LABELS[@]}" -gt 0 ]; then
    echo "${RED}✖ FALLÓ (${#FAIL_LABELS[@]}):${RESET}"
    local i
    for ((i=0; i<${#FAIL_LABELS[@]}; i++)); do
      echo "  - ${FAIL_LABELS[$i]}"
      echo "${FAIL_DETAILS[$i]}" | sed 's/^/      /'
    done
  fi
  echo "${DIM}Logs en: ${LOG_DIR}${RESET}"
  echo "${BOLD}=================================================${RESET}"
  echo
}

main() {
  ensure_tools
  build_groups "$@"   # opcional: pasa nombres de paquetes para filtrar
  run_groups
  print_summary
  exit "${EXIT_CODE}"
}

main "$@"