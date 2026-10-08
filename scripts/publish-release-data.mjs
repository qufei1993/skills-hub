import { S3Client, PutObjectCommand, GetObjectCommand } from '@aws-sdk/client-s3';
import { collectReleaseSnapshot } from './collect-release-data.mjs';
import { buildPublication, publishPublication, restorePublication } from './lib/release-publication.mjs';

async function main() {
  const account = process.env.CLOUDFLARE_ACCOUNT_ID;
  const accessKeyId = process.env.RELEASES_R2_ACCESS_KEY_ID;
  const secretAccessKey = process.env.RELEASES_R2_SECRET_ACCESS_KEY;
  if (!/^[a-f0-9]{32}$/.test(account || '') || !accessKeyId || !secretAccessKey) throw Error('Missing release storage configuration');
  const bucket = 'skills-hub-release-data';
  const client = new S3Client({ region: 'auto', endpoint: `https://${account}.r2.cloudflarestorage.com`, credentials: { accessKeyId, secretAccessKey }, requestChecksumCalculation: 'WHEN_REQUIRED', responseChecksumValidation: 'WHEN_REQUIRED' });
  const storage = {
    put: (Key, Body, options) => client.send(new PutObjectCommand({ Bucket: bucket, Key, Body, ContentType: options.contentType, CacheControl: options.cacheControl })),
    get: async Key => {
      const object = await client.send(new GetObjectCommand({ Bucket: bucket, Key }));
      return object.Body.transformToString();
    },
  };
  const rollback = process.env.RELEASES_ROLLBACK_SNAPSHOT;
  if (rollback) {
    const version = await restorePublication(rollback, storage);
    console.log(`Restored release data: ${version}`);
    return;
  }
  const fetchImpl = (url, init) => fetch(url, { ...init, headers: { ...init?.headers, ...(process.env.GH_TOKEN ? { Authorization: `Bearer ${process.env.GH_TOKEN}` } : {}) } });
  const snapshot = await collectReleaseSnapshot({ fetchImpl, sourceRef: 'latest' });
  const publication = buildPublication(snapshot);
  await publishPublication(publication, storage);
  console.log(`Published release data: ${publication.pointer.latest} (${publication.pointer.snapshotPath})`);
}
main().catch(() => { console.error('Release data publication failed. Existing current pointer was retained unless the final write completed. Check configuration and retry.'); process.exitCode = 1; });
