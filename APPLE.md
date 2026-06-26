
## macOS Signing und Notarization

Für eine normale macOS-Installation ohne Gatekeeper-Warnung braucht Mira ein
bezahltes Apple Developer Program. Ohne diesen Account kann das DMG zwar gebaut
werden, macOS behandelt es aber als nicht verifizierte App.

## Installation ohne Apple Account

Bis Signing und Notarization eingerichtet sind, werden die macOS-DMGs unsigned
ausgeliefert. Der Release-Workflow baut trotzdem weiter und legt
`install-macos.sh` neben die macOS-Dateien.

Mira besteht auf macOS aktuell aus zwei Apps:

- `Mira Installer.app`
- `Mira Client.app`

Beide DMGs können mit dem Script installiert werden:

```bash
chmod +x install-macos.sh
./install-macos.sh mira-installer-*.dmg mira-client-*.dmg
```

Für unsigned Testbuilds kann zusätzlich die Quarantine entfernt werden:

```bash
./install-macos.sh --allow-unsigned mira-installer-*.dmg mira-client-*.dmg
```

Das macht die Apps nicht Apple-notarized. Es kopiert nur die `.app` Bundles nach
`/Applications` und entfernt optional das Quarantine-Attribut, damit Tester die
Apps einfacher starten können.

Apple Developer Program:
- Kostet aktuell 99 USD pro Jahr bzw. lokale Währung.
- Für eine Firma sollte ein Organization Account genutzt werden. Dafür braucht
  Apple in der Regel eine D-U-N-S Number.
- Für private Releases reicht technisch ein Individual Account, dann erscheint
  aber der persönliche Entwicklername.

Benötigte GitHub Secrets:

```text
APPLE_CERTIFICATE
APPLE_CERTIFICATE_PASSWORD
APPLE_ID
APPLE_PASSWORD
APPLE_TEAM_ID
APPLE_SIGNING_IDENTITY
```

`APPLE_SIGNING_IDENTITY` ist optional, aber empfohlen.

### Apple Account erstellen

1. Auf https://developer.apple.com/programs/enroll/ anmelden.
2. Individual oder Organization auswählen.
3. Apple Developer Program kaufen und aktivieren.

### Developer ID Application Zertifikat erstellen

Diese Schritte müssen auf einem Mac gemacht werden:

1. `Keychain Access` öffnen.
2. `Certificate Assistant > Request a Certificate From a Certificate Authority`
   auswählen.
3. Email eintragen, `Saved to disk` wählen und eine `.certSigningRequest` Datei
   speichern.
4. In Apple Developer bei `Certificates, IDs & Profiles` ein neues Zertifikat
   erstellen.
5. Zertifikattyp `Developer ID Application` auswählen.
6. Die `.certSigningRequest` hochladen.
7. Das `.cer` Zertifikat herunterladen und per Doppelklick in die Keychain
   importieren.
8. Signing Identity prüfen:

```bash
security find-identity -v -p codesigning
```

Beispielwert für `APPLE_SIGNING_IDENTITY`:

```text
Developer ID Application: Mira Games GmbH (TEAMID)
```

### Zertifikat für GitHub Actions exportieren

1. In `Keychain Access` das `Developer ID Application` Zertifikat aufklappen.
2. Zertifikat inklusive Private Key als `.p12` exportieren.
3. Beim Export ein starkes Passwort setzen.
4. Base64 für GitHub erzeugen:

```bash
openssl base64 -A -in DeveloperIDApplication.p12 -out certificate-base64.txt
```

GitHub Secrets:

- `APPLE_CERTIFICATE`: Inhalt von `certificate-base64.txt`
- `APPLE_CERTIFICATE_PASSWORD`: Passwort vom `.p12` Export
- `APPLE_SIGNING_IDENTITY`: Ausgabe von `security find-identity`

### Notarization Credentials

1. Auf https://appleid.apple.com/ ein app-spezifisches Passwort erzeugen.
2. Team ID im Apple Developer Account unter Membership Details nachschauen.

GitHub Secrets:

- `APPLE_ID`: Apple Account Email
- `APPLE_PASSWORD`: app-spezifisches Passwort, nicht das normale Apple Passwort
- `APPLE_TEAM_ID`: Apple Developer Team ID

Wenn diese Secrets gesetzt sind, signiert und notarisiert der GitHub
Release-Workflow die macOS DMGs automatisch über Tauri.

Referenzen:
- https://developer.apple.com/programs/enroll/
- https://developer.apple.com/support/compare-memberships/
- https://v2.tauri.app/distribute/sign/macos/
