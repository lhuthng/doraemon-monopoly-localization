const databaseName = 'doraemon-monopoly-translator';
const storeName = 'files';
const workKey = 'work';

function database() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(databaseName, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(storeName);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

export async function readLocalFile(name) {
  const db = await database();
  return new Promise((resolve, reject) => {
    const request = db.transaction(storeName).objectStore(storeName).get(name);
    request.onsuccess = () => {
      const value = request.result;
      resolve(value instanceof Uint8Array || value instanceof ArrayBuffer ? new Uint8Array(value) : value);
    };
    request.onerror = () => reject(request.error);
  });
}

export async function saveLocalFile(name, bytes) {
  const db = await database();
  return new Promise((resolve, reject) => {
    const request = db.transaction(storeName, 'readwrite').objectStore(storeName).put(bytes, name);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
}

export async function clearLocalFiles() {
  const db = await database();
  return new Promise((resolve, reject) => {
    const request = db.transaction(storeName, 'readwrite').objectStore(storeName).clear();
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
}

export async function saveLocalWork(bytes, savedAt = Date.now()) {
  const db = await database();
  return new Promise((resolve, reject) => {
    const request = db
      .transaction(storeName, 'readwrite')
      .objectStore(storeName)
      .put({ bytes, savedAt }, workKey);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
}

export async function readLocalWork() {
  const db = await database();
  return new Promise((resolve, reject) => {
    const request = db.transaction(storeName).objectStore(storeName).get(workKey);
    request.onsuccess = () => {
      const value = request.result;
      if (!value) {
        resolve(null);
        return;
      }
      const bytes =
        value.bytes instanceof Uint8Array || value.bytes instanceof ArrayBuffer
          ? new Uint8Array(value.bytes)
          : value.bytes;
      resolve({ bytes, savedAt: typeof value.savedAt === 'number' ? value.savedAt : null });
    };
    request.onerror = () => reject(request.error);
  });
}
