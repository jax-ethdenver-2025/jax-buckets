// API endpoints
const API = {
    SHARE: '/api/v0/share',
    PROBE: '/api/v0/probe',
    QUERY: '/api/v0/query',
    CREATE_POOL: '/api/v0/pool'
};

// Helper to show messages
function showMessage(elementId, message, isError = false) {
    const el = document.getElementById(elementId);
    el.textContent = message;
    el.className = `message ${isError ? 'error' : 'success'}`;
}

// // Share form handler
// document.querySelector('#share-form')?.addEventListener('submit', async (e) => {
//     e.preventDefault();
//     const path = e.target.path.value;
//     const createPool = e.target.create_pool.checked;
//     const value = e.target.value.value;

//     try {
//         // Share the file
//         const shareRes = await fetch(API.SHARE, {
//             method: 'POST',
//             headers: {'Content-Type': 'application/json'},
//             body: JSON.stringify({ path })
//         });
//         const shareData = await shareRes.json();

//         if (!shareRes.ok) throw new Error(shareData.error);

//         let message = `File shared successfully!\nHash: ${shareData.hash}`;

//         // Create pool if requested
//         if (createPool) {
//             const poolRes = await fetch(API.CREATE_POOL, {
//                 method: 'POST',
//                 headers: {'Content-Type': 'application/json'},
//                 body: JSON.stringify({ hash: shareData.hash })
//             });
//             const poolData = await poolRes.json();

//             if (!poolRes.ok) throw new Error(poolData.error);
//             message += '\nPool created successfully!';
//         }

//         showMessage('share-message', message);
//     } catch (err) {
//         showMessage('share-message', err.message, true);
//     }
// });

// Query form handler
document.querySelector('#query-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const hash = e.target.hash.value;

    try {
        const res = await fetch(API.QUERY + '/' + hash, {
            method: 'GET',
            headers: {'Content-Type': 'application/json'}
        });
        const data = await res.json();

        if (!res.ok) throw new Error(data.error);

        const resultsDiv = document.getElementById('query-results');
        if (data.nodes.length === 0) {
            resultsDiv.innerHTML = '<p>No known locations</p>';
        } else {
            resultsDiv.innerHTML = `
                <table>
                    <thead>
                        <tr>
                            <th>Node ID</th>
                            <th>Trust Score</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${data.nodes.map(([node, trust]) => `
                            <tr>
                                <td>${node}</td>
                                <td>${trust.toFixed(3)}</td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            `;
        }
        showMessage('query-message', 'Query completed successfully');
    } catch (err) {
        showMessage('query-message', err.message, true);
    }
});

async function queryContent(hash) {
    try {
        console.log('Querying content for hash:', hash);
        console.log('API.QUERY:', API.QUERY);
        console.log('API.QUERY + "/" + hash:', API.QUERY + '/' + hash);
        const res = await fetch(API.QUERY + '/' + hash, {
            method: 'GET',
            headers: {'Content-Type': 'application/json'}
        });
        const data = await res.json();
        
        if (!res.ok) throw new Error(data.error);
        
        // Redirect to query page with results
        window.location.href = `/query?hash=${hash}`;
    } catch (err) {
        console.error('Query failed:', err);
        alert(err.message);
    }
} 