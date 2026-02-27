# GitHub Repository Setup Checklist

Complete these steps to enable the full CI/CD pipeline for GrabMe.

## ✅ Local Setup (Already Done)

- [x] LICENSE files added (MIT and Apache-2.0)
- [x] Cargo.toml manifests hardened with crates.io metadata
- [x] Publishable crates identified and configured
- [x] Internal crates marked with `publish = false`
- [x] GitHub Actions workflows created (ci.yml, release.yml, dist.yml)
- [x] Release documentation (RELEASING.md, DEPLOYMENT.md)

## 🔧 GitHub Repository Settings (Action Required)

### 1. Create crates.io API Token

1. Go to https://crates.io/settings/tokens
2. Click "New Token"
3. Name: `GrabMe GitHub Actions`
4. Scope: `publish-update`
5. Click "Create"
6. **Copy the token immediately** (it won't be shown again)

### 2. Add GitHub Secret

1. Go to your GitHub repository
2. Navigate to **Settings → Secrets and variables → Actions**
3. Click **New repository secret**
4. Name: `CARGO_REGISTRY_TOKEN`
5. Value: Paste the crates.io token from step 1
6. Click **Add secret**

### 3. Create Release Environment (Optional but Recommended)

For additional safety:

1. Go to **Settings → Environments**
2. Click **New environment**
3. Name: `release`
4. Add protection rules:
   - ☑ Required reviewers (add team members)
   - ☑ Wait timer: 5 minutes (optional)
5. Click **Save protection rules**

This requires manual approval before publishing to crates.io.

### 4. Enable GitHub Actions

1. Go to **Settings → Actions → General**
2. Under "Actions permissions":
   - Select **Allow all actions and reusable workflows**
3. Under "Workflow permissions":
   - Select **Read and write permissions**
   - ☑ Check **Allow GitHub Actions to create and approve pull requests**
4. Click **Save**

### 5. Configure Branch Protection (Optional)

Protect the `main` branch:

1. Go to **Settings → Branches**
2. Click **Add branch protection rule**
3. Branch name pattern: `main`
4. Enable:
   - ☑ Require a pull request before merging
   - ☑ Require status checks to pass before merging
     - Add required checks:
       - `Rustfmt`
       - `Clippy`
       - `Test Suite`
       - `Cargo Check`
       - `Package Integrity`
   - ☑ Require conversation resolution before merging
5. Click **Create**

## 🚀 Test the Pipeline

### Test CI (Automated on every commit)

1. Create a test branch: `git checkout -b test-ci`
2. Make a trivial change: `echo "# Test" >> README.md`
3. Commit and push: `git add . && git commit -m "test: CI pipeline" && git push origin test-ci`
4. Create a PR on GitHub
5. Watch CI workflows run: https://github.com/velocitatem/grabme/actions

Expected results:
- ✅ Rustfmt check passes
- ✅ Clippy check passes
- ✅ Test suite passes
- ✅ Cargo check passes
- ✅ Package integrity passes

### Test Release (Manual trigger)

⚠️ **Warning**: This will publish to crates.io! Only do this when ready for `v0.1.0`.

1. Ensure `main` branch is clean and CI passes
2. Create and push a tag:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. Watch workflows at: https://github.com/velocitatem/grabme/actions
4. Verify:
   - ✅ Crates published at https://crates.io/crates/grabme-cli
   - ✅ GitHub Release created at https://github.com/velocitatem/grabme/releases
   - ✅ Linux binary attached to release

### Dry-run (Test Without Publishing)

To test release workflow without actually publishing:

1. Comment out `cargo publish` lines in `.github/workflows/release.yml`
2. Replace with `cargo package -p <crate> --allow-dirty --no-verify`
3. Push to a test branch and trigger workflow manually

## 📊 Status Badges

Add to `README.md`:

```markdown
[![CI](https://github.com/velocitatem/grabme/actions/workflows/ci.yml/badge.svg)](https://github.com/velocitatem/grabme/actions/workflows/ci.yml)
[![Release](https://github.com/velocitatem/grabme/actions/workflows/release.yml/badge.svg)](https://github.com/velocitatem/grabme/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/grabme-cli.svg)](https://crates.io/crates/grabme-cli)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
```

## 🔍 Troubleshooting

### "Permission denied" errors in CI

- Ensure **Workflow permissions** are set to "Read and write"
- Check that GITHUB_TOKEN has sufficient permissions

### "cargo publish" fails with authentication error

- Verify `CARGO_REGISTRY_TOKEN` secret is set correctly
- Regenerate token on crates.io if needed
- Ensure token has `publish-update` scope

### Crate already exists on crates.io

- Workflows use `continue-on-error: true` to skip already-published crates
- This is expected behavior for re-runs

### Missing system dependencies in CI

- All required deps are in `.github/workflows/ci.yml`
- If builds fail, check workflow logs for missing packages

## 📝 Next Steps After Setup

1. **First release**: Tag and release `v0.1.0`
2. **Monitor workflows**: Watch Actions tab for first few releases
3. **Update README**: Add badges and installation instructions
4. **Announce**: Share release on social media/forums
5. **Documentation**: Ensure docs.rs renders correctly

## 🆘 Need Help?

- Check workflow logs: https://github.com/velocitatem/grabme/actions
- Review `RELEASING.md` for release process
- Review `DEPLOYMENT.md` for architecture overview
- Open an issue: https://github.com/velocitatem/grabme/issues

---

**Checklist completion**: 5/5 local tasks ✅ | 0/5 GitHub tasks ⏳
