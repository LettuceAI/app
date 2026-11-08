#!/usr/bin/env bash
set -euo pipefail
umask 077

output=${1:?Usage: bash scripts/macos/create-signing-identity.sh OUTPUT_DIRECTORY}
# Refuse to replace an identity: existing Keychain grants depend on this certificate.
mkdir "$output"
openssl rand -base64 48 | tr -d '\n' > "$output/password.txt"
cat > "$output/certificate.cnf" <<'CONF'
[req]
distinguished_name = subject
x509_extensions = codesign
prompt = no
[subject]
CN = LettuceAI Code Signing
[codesign]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always
CONF
openssl req -new -x509 -newkey rsa:3072 -sha256 -days 3650 \
  -config "$output/certificate.cnf" \
  -passout "file:$output/password.txt" \
  -keyout "$output/private-key.pem" -out "$output/certificate.pem" 2>/dev/null
# These PBE algorithms are supported by macOS's PKCS#12 importer.
LETTUCE_P12_PASSWORD=$(cat "$output/password.txt") \
openssl pkcs12 -export -name 'LettuceAI Code Signing' \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
  -inkey "$output/private-key.pem" -passin "file:$output/password.txt" \
  -in "$output/certificate.pem" -out "$output/certificate.p12" \
  -passout env:LETTUCE_P12_PASSWORD
openssl base64 -A -in "$output/certificate.p12" -out "$output/certificate.base64"
openssl x509 -in "$output/certificate.pem" -noout -fingerprint -sha1 \
  | cut -d= -f2 | tr -d ':' > "$output/fingerprint.txt"
printf 'Created a private signing identity in %s. Keep it for every release.\n' "$output"
printf 'Public certificate fingerprint: '
cat "$output/fingerprint.txt"
