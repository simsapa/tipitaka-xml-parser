// Fragment Review Application

// Application state
let state = {
    selectedFile: null,
    selectedFragmentId: null,
    fragments: []
};

// Initialize the application
function init() {
    fetchAndPopulateFileList();
    setupEventListeners();
    setupResizablePanels();
    console.log('Fragment Review Application initialized');
}

// Fetch and populate file list from API
async function fetchAndPopulateFileList() {
    const fileList = document.getElementById('file-list');
    fileList.innerHTML = '<div class="panel-item">Loading...</div>';
    
    try {
        const response = await fetch('/api/files');
        if (!response.ok) throw new Error('Failed to fetch files');
        
        const files = await response.json();
        fileList.innerHTML = '';
        
        files.forEach(file => {
            const item = document.createElement('div');
            item.className = 'panel-item';
            item.textContent = `${file.filename} (${file.fragment_count})`;
            item.dataset.filename = file.filename;
            item.onclick = () => selectFile(file.filename);
            fileList.appendChild(item);
        });
        
        if (files.length === 0) {
            fileList.innerHTML = '<div class="panel-item">No files found</div>';
        }
    } catch (error) {
        console.error('Error fetching files:', error);
        fileList.innerHTML = '<div class="panel-item has-text-danger">Error loading files</div>';
    }
}

// Select a file and populate fragments
async function selectFile(filename) {
    state.selectedFile = filename;
    
    // Update UI: highlight selected file
    document.querySelectorAll('#file-list .panel-item').forEach(item => {
        item.classList.toggle('is-active', item.dataset.filename === filename);
    });
    
    await fetchAndPopulateFragmentList(filename);
    clearFragmentDetails();
}

// Fetch and populate fragment list from API
async function fetchAndPopulateFragmentList(filename) {
    const fragmentList = document.getElementById('fragment-list');
    fragmentList.innerHTML = '<div class="panel-item">Loading...</div>';
    
    try {
        const response = await fetch(`/api/files/${encodeURIComponent(filename)}/fragments`);
        if (!response.ok) throw new Error('Failed to fetch fragments');
        
        const fragments = await response.json();
        state.fragments = fragments;
        
        fragmentList.innerHTML = '';
        
        fragments.forEach(fragment => {
            const item = document.createElement('div');
            item.className = 'panel-item';
            
            const statusColor = getStatusColor(fragment.frag_review);
            item.innerHTML = `
                <span class="tag is-${statusColor} is-small">${fragment.frag_type}</span>
                <span>Fragment ${fragment.frag_idx}</span>
            `;
            item.dataset.fragmentId = fragment.id;
            item.onclick = () => selectFragment(fragment.id);
            fragmentList.appendChild(item);
        });
        
        if (fragments.length === 0) {
            fragmentList.innerHTML = '<div class="panel-item">No fragments found</div>';
        }
    } catch (error) {
        console.error('Error fetching fragments:', error);
        fragmentList.innerHTML = '<div class="panel-item has-text-danger">Error loading fragments</div>';
    }
}

// Get Bulma color class for review status
function getStatusColor(status) {
    if (!status) return 'light';
    
    const colorMap = {
        'unchecked': 'light',
        'in-progress': 'warning',
        'checked': 'success',
        'needs-review': 'danger'
    };
    return colorMap[status] || 'light';
}

// Select a fragment and show details
async function selectFragment(fragmentId) {
    state.selectedFragmentId = fragmentId;
    
    // Update UI: highlight selected fragment
    document.querySelectorAll('#fragment-list .panel-item').forEach(item => {
        item.classList.toggle('is-active', parseInt(item.dataset.fragmentId) === fragmentId);
    });
    
    await fetchAndDisplayFragmentDetails(fragmentId);
}

// Fetch and display fragment details from API
async function fetchAndDisplayFragmentDetails(fragmentId) {
    try {
        const response = await fetch(`/api/fragments/${fragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        
        const detail = await response.json();
        
        // Update metadata fields
        document.getElementById('frag_type').value = detail.frag_type;
        document.getElementById('frag_review').value = detail.frag_review || '';
        document.getElementById('cst_code').value = detail.cst_code || '';
        document.getElementById('cst_vagga').value = detail.cst_vagga || '';
        document.getElementById('cst_sutta').value = detail.cst_sutta || '';
        document.getElementById('sc_code').value = detail.sc_code || '';
        
        // Update text areas
        document.getElementById('prev-content').value = detail.prev_fragment ? detail.prev_fragment.content_xml : '';
        document.getElementById('current-content').value = detail.content_xml;
        document.getElementById('next-content').value = detail.next_fragment ? detail.next_fragment.content_xml : '';
        
        // Enable/disable controls based on position
        const hasPrev = detail.prev_fragment !== null;
        const hasNext = detail.next_fragment !== null;
        
        document.getElementById('delete-prev-btn').disabled = !hasPrev;
        document.querySelectorAll('[id^="prev-"]').forEach(btn => btn.disabled = !hasPrev);
        
        document.getElementById('delete-next-btn').disabled = !hasNext;
        document.querySelectorAll('[id^="next-"]').forEach(btn => btn.disabled = !hasNext);
        
    } catch (error) {
        console.error('Error fetching fragment details:', error);
        alert('Error loading fragment details');
    }
}

// Clear fragment details
function clearFragmentDetails() {
    document.getElementById('frag_type').value = '';
    document.getElementById('frag_review').value = '';
    document.getElementById('cst_code').value = '';
    document.getElementById('cst_vagga').value = '';
    document.getElementById('cst_sutta').value = '';
    document.getElementById('sc_code').value = '';
    
    document.getElementById('prev-content').value = '';
    document.getElementById('current-content').value = '';
    document.getElementById('next-content').value = '';
}

// Update fragment metadata (auto-save on blur)
async function updateFragmentMetadata() {
    if (!state.selectedFragmentId) return;
    
    const metadata = {
        frag_review: document.getElementById('frag_review').value || null,
        cst_code: document.getElementById('cst_code').value || null,
        sc_code: document.getElementById('sc_code').value || null,
        cst_vagga: document.getElementById('cst_vagga').value || null,
        cst_sutta: document.getElementById('cst_sutta').value || null,
        cst_paranum: null,
        sc_sutta: null,
    };
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(metadata)
        });
        
        if (!response.ok) throw new Error('Failed to update metadata');
        
        // Refresh fragment list to show updated review status
        await fetchAndPopulateFragmentList(state.selectedFile);
        
        console.log('Metadata updated successfully');
    } catch (error) {
        console.error('Error updating metadata:', error);
        alert('Failed to save metadata changes');
    }
}

// Adjust fragment boundary
async function adjustBoundary(action, direction) {
    if (!state.selectedFragmentId) return;
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}/adjust-boundary`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ action, direction })
        });
        
        if (!response.ok) throw new Error('Failed to adjust boundary');
        
        const result = await response.json();
        
        // Refresh the current fragment view
        await fetchAndDisplayFragmentDetails(state.selectedFragmentId);
        
        console.log('Boundary adjusted:', result.message);
    } catch (error) {
        console.error('Error adjusting boundary:', error);
        alert('Failed to adjust boundary');
    }
}

// Delete fragment
async function deleteFragment(direction) {
    if (!state.selectedFragmentId) return;
    
    // Determine which fragment to delete
    const detail = await getCurrentFragmentDetail();
    if (!detail) return;
    
    let fragmentIdToDelete;
    if (direction === 'prev' && detail.prev_fragment) {
        fragmentIdToDelete = detail.prev_fragment.id;
    } else if (direction === 'next' && detail.next_fragment) {
        fragmentIdToDelete = detail.next_fragment.id;
    } else {
        return;
    }
    
    try {
        const response = await fetch(`/api/fragments/${fragmentIdToDelete}`, {
            method: 'DELETE'
        });
        
        if (!response.ok) throw new Error('Failed to delete fragment');
        
        // Refresh fragment list and reload current fragment
        await fetchAndPopulateFragmentList(state.selectedFile);
        await fetchAndDisplayFragmentDetails(state.selectedFragmentId);
        
        console.log('Fragment deleted successfully');
    } catch (error) {
        console.error('Error deleting fragment:', error);
        alert('Failed to delete fragment');
    }
}

// Get current fragment detail
async function getCurrentFragmentDetail() {
    if (!state.selectedFragmentId) return null;
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        return await response.json();
    } catch (error) {
        console.error('Error fetching fragment details:', error);
        return null;
    }
}

// Setup event listeners for buttons
function setupEventListeners() {
    // Auto-save metadata on blur
    document.getElementById('frag_review').onchange = updateFragmentMetadata;
    document.getElementById('cst_code').onblur = updateFragmentMetadata;
    document.getElementById('sc_code').onblur = updateFragmentMetadata;
    document.getElementById('cst_vagga').onblur = updateFragmentMetadata;
    document.getElementById('cst_sutta').onblur = updateFragmentMetadata;
    
    // Boundary adjustment buttons for previous fragment
    document.getElementById('prev-line-up').onclick = () => adjustBoundary('line_up', 'prev');
    document.getElementById('prev-line-down').onclick = () => adjustBoundary('line_down', 'prev');
    document.getElementById('prev-char-left').onclick = () => adjustBoundary('char_left', 'prev');
    document.getElementById('prev-char-right').onclick = () => adjustBoundary('char_right', 'prev');
    
    // Boundary adjustment buttons for next fragment
    document.getElementById('next-line-up').onclick = () => adjustBoundary('line_up', 'next');
    document.getElementById('next-line-down').onclick = () => adjustBoundary('line_down', 'next');
    document.getElementById('next-char-left').onclick = () => adjustBoundary('char_left', 'next');
    document.getElementById('next-char-right').onclick = () => adjustBoundary('char_right', 'next');
    
    // Delete buttons with confirmation
    document.getElementById('delete-prev-btn').onclick = () => {
        showConfirmModal('Are you sure you want to delete the previous fragment?', () => {
            deleteFragment('prev');
        });
    };
    
    document.getElementById('delete-next-btn').onclick = () => {
        showConfirmModal('Are you sure you want to delete the next fragment?', () => {
            deleteFragment('next');
        });
    };
    
    // Modal controls
    document.getElementById('modal-close').onclick = closeModal;
    document.getElementById('modal-cancel').onclick = closeModal;
}

// Show confirmation modal
function showConfirmModal(message, onConfirm) {
    document.getElementById('modal-message').textContent = message;
    document.getElementById('modal-confirm').onclick = () => {
        onConfirm();
        closeModal();
    };
    document.getElementById('confirm-modal').classList.add('is-active');
}

// Close modal
function closeModal() {
    document.getElementById('confirm-modal').classList.remove('is-active');
}

// Setup resizable panels with draggable separator
function setupResizablePanels() {
    const separator = document.getElementById('separator');
    const leftPanel = document.getElementById('left-panel');
    const rightPanel = document.getElementById('right-panel');
    const container = document.getElementById('main-container');
    
    let isResizing = false;
    
    separator.addEventListener('mousedown', (e) => {
        isResizing = true;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        e.preventDefault();
    });
    
    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;
        
        const containerRect = container.getBoundingClientRect();
        const newLeftWidth = e.clientX - containerRect.left;
        
        // Enforce min/max constraints
        if (newLeftWidth >= 250 && newLeftWidth <= 600) {
            leftPanel.style.flex = `0 0 ${newLeftWidth}px`;
        }
    });
    
    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
