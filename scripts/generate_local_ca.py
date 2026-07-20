#!/usr/bin/env python3
"""Generate a local CA + server certificate pair for Clarity Gateway TLS.

Unlike the previous self-signed localhost.crt, this creates a dedicated local
CA. Devices that install the CA certificate (e.g. Android/iOS in the same LAN)
will trust the server certificate automatically, avoiding the "self-signed cert"
warning.

Outputs under .clarity/:
  - local-ca.crt      : local CA certificate (install this on phones/clients)
  - local-ca.key      : local CA private key (keep secret)
  - localhost.crt     : server certificate signed by the CA
  - localhost.key     : server private key

The server cert covers:
  - DNS: localhost, {hostname}
  - IP: 127.0.0.1, {local_ip}

Usage:
    python scripts/generate_local_ca.py
"""

import datetime
import socket
import sys
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID

UTC = datetime.timezone.utc

CLARITY_DIR = Path(".clarity")
CA_CERT_FILE = CLARITY_DIR / "local-ca.crt"
CA_KEY_FILE = CLARITY_DIR / "local-ca.key"
SERVER_CERT_FILE = CLARITY_DIR / "localhost.crt"
SERVER_KEY_FILE = CLARITY_DIR / "localhost.key"


def generate_key():
    return rsa.generate_private_key(public_exponent=65537, key_size=2048)


def generate_ca():
    key = generate_key()
    subject = issuer = x509.Name(
        [
            x509.NameAttribute(NameOID.COUNTRY_NAME, "CN"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "Clarity Local CA"),
            x509.NameAttribute(NameOID.COMMON_NAME, "Clarity Local Root CA"),
        ]
    )
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(issuer)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(datetime.datetime.now(UTC) - datetime.timedelta(days=1))
        .not_valid_after(datetime.datetime.now(UTC) + datetime.timedelta(days=365 * 5))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(
            x509.KeyUsage(
                digital_signature=False,
                content_commitment=False,
                key_encipherment=False,
                data_encipherment=False,
                key_agreement=False,
                key_cert_sign=True,
                crl_sign=True,
                encipher_only=False,
                decipher_only=False,
            ),
            critical=True,
        )
        .sign(key, hashes.SHA256())
    )
    return cert, key


def generate_server_cert(ca_cert, ca_key):
    key = generate_key()
    hostname = socket.gethostname()
    local_ip = socket.gethostbyname(hostname)

    subject = x509.Name(
        [
            x509.NameAttribute(NameOID.COUNTRY_NAME, "CN"),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "Clarity"),
            x509.NameAttribute(NameOID.COMMON_NAME, "localhost"),
        ]
    )

    san = x509.SubjectAlternativeName(
        [
            x509.DNSName("localhost"),
            x509.DNSName(hostname),
            x509.IPAddress(ip_address_from_string("127.0.0.1")),
            x509.IPAddress(ip_address_from_string(local_ip)),
        ]
    )

    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(ca_cert.subject)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(datetime.datetime.now(UTC) - datetime.timedelta(days=1))
        .not_valid_after(datetime.datetime.now(UTC) + datetime.timedelta(days=365))
        .add_extension(san, critical=False)
        .add_extension(
            x509.ExtendedKeyUsage([x509.ExtendedKeyUsageOID.SERVER_AUTH]),
            critical=False,
        )
        .sign(ca_key, hashes.SHA256())
    )
    return cert, key


def ip_address_from_string(s: str):
    import ipaddress
    return ipaddress.ip_address(s)


def write_pem(path: Path, data: bytes) -> None:
    path.write_bytes(data)


def main() -> int:
    CLARITY_DIR.mkdir(parents=True, exist_ok=True)

    if all(p.exists() for p in (CA_CERT_FILE, CA_KEY_FILE, SERVER_CERT_FILE, SERVER_KEY_FILE)):
        print("Local CA and server certificate already exist under .clarity/")
        print(f"  CA cert:   {CA_CERT_FILE}")
        print(f"  Server cert: {SERVER_CERT_FILE}")
        print("Run with --force to regenerate.")
        return 0

    print("Generating local CA...")
    ca_cert, ca_key = generate_ca()
    write_pem(CA_CERT_FILE, ca_cert.public_bytes(serialization.Encoding.PEM))
    write_pem(
        CA_KEY_FILE,
        ca_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.TraditionalOpenSSL,
            encryption_algorithm=serialization.NoEncryption(),
        ),
    )

    print("Generating server certificate signed by local CA...")
    server_cert, server_key = generate_server_cert(ca_cert, ca_key)
    write_pem(SERVER_CERT_FILE, server_cert.public_bytes(serialization.Encoding.PEM))
    write_pem(
        SERVER_KEY_FILE,
        server_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.TraditionalOpenSSL,
            encryption_algorithm=serialization.NoEncryption(),
        ),
    )

    print("\nDone. Files:")
    print(f"  CA cert (install on clients): {CA_CERT_FILE}")
    print(f"  Server cert (proxy uses):     {SERVER_CERT_FILE}")
    print(f"\nTo use the proxy:")
    print(f"  python scripts/tls_reverse_proxy.py")
    print(f"\nTo test from this machine:")
    print(f"  curl --cacert {CA_CERT_FILE} https://localhost:8443/health")
    print(f"\nTo trust on Android:")
    print(f"  Settings -> Security -> Install certificate from storage -> {CA_CERT_FILE.name}")
    print(f"\nTo trust on Windows (this machine):")
    print(f"  certutil -addstore -f ROOT {CA_CERT_FILE}")
    return 0


if __name__ == "__main__":
    if "--force" in sys.argv:
        for p in (CA_CERT_FILE, CA_KEY_FILE, SERVER_CERT_FILE, SERVER_KEY_FILE):
            p.unlink(missing_ok=True)
    sys.exit(main())
