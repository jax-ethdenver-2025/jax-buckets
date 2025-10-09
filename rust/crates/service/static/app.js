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

// Initialize modules when DOM is ready
document.addEventListener('DOMContentLoaded', function() {
    // Get API URL from data attribute on body or window
    const apiUrl = window.JAX_API_URL || 'http://localhost:3000';
    const bucketId = window.JAX_BUCKET_ID;

    BucketCreation.init(apiUrl);
    if (bucketId) {
        FileUpload.init(apiUrl, bucketId);
    }
});
