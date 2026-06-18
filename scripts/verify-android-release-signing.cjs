const fs = require('fs');

function fail(message) {
  console.error(`FAIL: ${message}`);
  process.exitCode = 1;
}

const workflow = fs.readFileSync('.github/workflows/release.yml', 'utf8');
const gradle = fs.readFileSync('src-tauri/gen/android/app/build.gradle.kts', 'utf8');

for (const token of [
  'Validate Android signing secrets',
  'ANDROID_KEYSTORE_BASE64',
  'ANDROID_KEYSTORE_PASSWORD',
  'ANDROID_KEY_ALIAS',
  'ANDROID_KEY_PASSWORD',
  'Decode Android keystore',
  'Prepare signed Android artifact',
  'Verify Android APK signature',
  'signed-release/*.apk',
]) {
  if (!workflow.includes(token)) {
    fail(`release workflow missing token: ${token}`);
  }
}

if (/push:\s*\r?\n(?:[^\n]*\r?\n){0,6}\s*branches:/m.test(workflow)) {
  fail('release workflow still runs on branch pushes; release builds must run only on tags or manual dispatch');
}
if (workflow.includes('outputs/apk/universal/release/*.apk')) {
  fail('release workflow still uploads raw universal release APK glob, which can include unsigned APKs');
}
if (!workflow.includes("! -name '*unsigned*'")) {
  fail('release workflow does not explicitly exclude unsigned APKs when preparing artifacts');
}

for (const token of [
  'import org.gradle.api.GradleException',
  'signingConfigs',
  'create("release")',
  'System.getenv("ANDROID_KEYSTORE_PATH")',
  'System.getenv("ANDROID_KEYSTORE_PASSWORD")',
  'System.getenv("ANDROID_KEY_ALIAS")',
  'System.getenv("ANDROID_KEY_PASSWORD")',
  'requestedReleaseBuild',
  'Android release signing is required',
  'signingConfig = signingConfigs.getByName("release")',
]) {
  if (!gradle.includes(token)) {
    fail(`Gradle signing config missing token: ${token}`);
  }
}

if (process.exitCode) {
  process.exit(process.exitCode);
}
console.log('PASS: Android release signing guardrails are configured');
