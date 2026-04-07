# Driver Signing for Production

## Current Status
The SYSVAD-based virtual audio driver (TabletAudioSample) is currently signed with
the WDK test certificate (WDKTestCert). This allows installation on test machines
with Secure Boot disabled and test signing enabled, but NOT on production systems.

## For Production Signing

### Option 1: Microsoft Partner Center (WHQL)
1. Create a Microsoft Partner Center account: https://partner.microsoft.com/dashboard
2. Submit the driver package for WHQL certification
3. Microsoft signs the driver with their cross-certificate chain
4. Cost: ~$500/year EV code signing certificate required

### Option 2: Extended Validation (EV) Code Signing
1. Obtain EV code signing certificate from DigiCert, Sectigo, or GlobalSign (~$300-500/year)
2. Sign the driver .sys file: `signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /a inferno_audio.sys`
3. For kernel-mode drivers, EV signing + WHQL is still required on Windows 10/11

### Option 3: Test Signing (Current — Dev/Test Only)
Enable test signing on the target machine:
```
bcdedit /set testsigning on
```
Then install with the WDKTestCert. NOT for production/distribution.

### Enabling Test Signing (Current Workflow)
```powershell
# The setup scripts in scripts/ handle this automatically
bcdedit /set testsigning on
# Install cert
certutil -addstore TrustedPublisher WDKTestCert.cer
# Install driver
pnputil /add-driver *.inf /install
```

## CI/CD Signing Integration
For automated signing in GitHub Actions, store the EV certificate as a GitHub secret
and use `signtool` in the release workflow.
