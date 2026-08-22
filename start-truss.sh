#!/bin/bash
export RUST_LOG=info
# arch native gbm — flake exports NixOS paths (/run/opengl-driver) that break here
export GBM_BACKENDS_PATH=/usr/lib/gbm
unset __NIX_DIRENV_PROFILE_ENV
exec &> /home/m57/truss.log
cd /home/m57
/home/m57/target-arch/debug/truss
echo "TRUSS-EXITED code=$?"
