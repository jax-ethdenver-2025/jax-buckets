// Bucket Creation Module
const BucketCreation = {
    init(apiUrl) {
        const form = document.getElementById('createBucketForm');
        if (!form) return;

        form.addEventListener('submit', async (e) => {
            e.preventDefault();

            const nameInput = document.getElementById('bucketName');
            const status = document.getElementById('createStatus');
            const name = nameInput.value.trim();

            if (!name) {
                this.showStatus(status, 'Please enter a bucket name', 'error');
                return;
            }

            this.showStatus(status, 'Creating bucket...', 'info');

            try {
                const response = await fetch(`${apiUrl}/api/v0/bucket`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: name })
                });

                if (response.ok) {
                    this.showStatus(status, 'Bucket created successfully! Reloading...', 'success');
                    setTimeout(() => window.location.reload(), 1000);
                } else {
                    const error = await response.text();
                    this.showStatus(status, 'Failed to create bucket: ' + error, 'error');
                }
            } catch (error) {
                this.showStatus(status, 'Failed to create bucket: ' + error.message, 'error');
            }
        });
    },

    showStatus(element, message, type) {
        element.className = 'p-4 ' + (type === 'error' ? 'bg-red-100 text-red-800' :
                                      type === 'success' ? 'bg-green-100 text-green-800' :
                                      'bg-blue-100 text-blue-800');
        element.textContent = message;
        element.classList.remove('hidden');
    }
};

// File Upload Module
const FileUpload = {
    init(apiUrl, bucketId) {
        const form = document.getElementById('uploadForm');
        if (!form) return;

        form.addEventListener('submit', async (e) => {
            e.preventDefault();

            const fileInput = document.getElementById('fileInput');
            const pathInput = document.getElementById('pathInput');
            const status = document.getElementById('uploadStatus');

            if (!fileInput.files.length) {
                this.showStatus(status, 'Please select a file', 'error');
                return;
            }

            const file = fileInput.files[0];
            const path = pathInput.value || '/';

            // Construct mount_path: join directory path with filename
            const mountPath = path.endsWith('/') ? path + file.name : path + '/' + file.name;

            const formData = new FormData();
            formData.append('bucket_id', bucketId);
            formData.append('mount_path', mountPath);
            formData.append('file', file);

            this.showStatus(status, 'Uploading...', 'info');

            try {
                const response = await fetch(`${apiUrl}/api/v0/bucket/add`, {
                    method: 'POST',
                    body: formData
                });

                if (response.ok) {
                    this.showStatus(status, 'File uploaded successfully! Reloading...', 'success');
                    setTimeout(() => window.location.reload(), 1000);
                } else {
                    const error = await response.text();
                    this.showStatus(status, 'Upload failed: ' + error, 'error');
                }
            } catch (error) {
                this.showStatus(status, 'Upload failed: ' + error.message, 'error');
            }
        });
    },

    showStatus(element, message, type) {
        element.className = 'p-4 ' + (type === 'error' ? 'bg-red-100 text-red-800' :
                                      type === 'success' ? 'bg-green-100 text-green-800' :
                                      'bg-blue-100 text-blue-800');
        element.textContent = message;
        element.classList.remove('hidden');
    }
};

// Bucket Share Module
const BucketShare = {
    init(apiUrl, bucketId) {
        const form = document.getElementById('shareForm');
        if (!form) return;

        form.addEventListener('submit', async (e) => {
            e.preventDefault();

            const peerPublicKeyInput = document.getElementById('peerPublicKeyInput');
            const status = document.getElementById('shareStatus');
            const peerPublicKey = peerPublicKeyInput.value.trim();

            if (!peerPublicKey) {
                this.showStatus(status, 'Please enter a peer public key', 'error');
                return;
            }

            // Validate hex format (64 characters)
            if (!/^[a-fA-F0-9]{64}$/.test(peerPublicKey)) {
                this.showStatus(status, 'Invalid public key format. Must be 64 hexadecimal characters.', 'error');
                return;
            }

            this.showStatus(status, 'Sharing bucket...', 'info');

            try {
                const response = await fetch(`${apiUrl}/api/v0/bucket/share`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        bucket_id: bucketId,
                        peer_public_key: peerPublicKey
                    })
                });

                if (response.ok) {
                    this.showStatus(status, 'Bucket shared successfully!', 'success');
                    setTimeout(() => {
                        peerPublicKeyInput.value = '';
                        status.classList.add('hidden');
                        UIkit.modal('#share-modal').hide();
                    }, 1500);
                } else {
                    const error = await response.text();
                    this.showStatus(status, 'Failed to share bucket: ' + error, 'error');
                }
            } catch (error) {
                this.showStatus(status, 'Failed to share bucket: ' + error.message, 'error');
            }
        });
    },

    showStatus(element, message, type) {
        element.className = 'p-4 ' + (type === 'error' ? 'bg-red-100 text-red-800' :
                                      type === 'success' ? 'bg-green-100 text-green-800' :
                                      'bg-blue-100 text-blue-800');
        element.textContent = message;
        element.classList.remove('hidden');
    }
};

// Initialize modules when DOM is ready
document.addEventListener('DOMContentLoaded', function() {
    // Get API URL from data attribute on body or window
    const apiUrl = window.JAX_API_URL || 'http://localhost:3000';
    const bucketId = window.JAX_BUCKET_ID;

    BucketCreation.init(apiUrl);
    if (bucketId) {
        FileUpload.init(apiUrl, bucketId);
        BucketShare.init(apiUrl, bucketId);
    }
});
