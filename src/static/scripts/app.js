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

// Setup event listeners for buttons
function setupEventListeners() {
    // Boundary adjustment buttons (placeholders for Stage 3)
    document.getElementById('prev-line-up').onclick = () => console.log('Previous: Line Up');
    document.getElementById('prev-line-down').onclick = () => console.log('Previous: Line Down');
    document.getElementById('prev-char-left').onclick = () => console.log('Previous: Char Left');
    document.getElementById('prev-char-right').onclick = () => console.log('Previous: Char Right');
    
    document.getElementById('next-line-up').onclick = () => console.log('Next: Line Up');
    document.getElementById('next-line-down').onclick = () => console.log('Next: Line Down');
    document.getElementById('next-char-left').onclick = () => console.log('Next: Char Left');
    document.getElementById('next-char-right').onclick = () => console.log('Next: Char Right');
    
    // Delete buttons (placeholders for Stage 3)
    document.getElementById('delete-prev-btn').onclick = () => showConfirmModal('Are you sure you want to delete the previous fragment?', () => {
        console.log('Delete previous fragment');
    });
    
    document.getElementById('delete-next-btn').onclick = () => showConfirmModal('Are you sure you want to delete the next fragment?', () => {
        console.log('Delete next fragment');
    });
    
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
