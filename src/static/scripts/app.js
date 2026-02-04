// Fragment Review Application

// Application state
let state = {
    selectedFile: null,
    selectedFragmentId: null,
    fragments: []
};

// ArangoDB connection state
window.paliTitlesCache = null;  // Cache for Pali titles from ArangoDB (uid -> name mapping)
window.arangoConnected = false; // Track ArangoDB connection state

// Validation state
let validationResults = null;   // Results from last validation run
let selectedCheckType = null;   // Currently selected check type ID

// Initialize the application
function init() {
    loadStateFromLocalStorage();
    fetchAndPopulateFileList();
    setupEventListeners();
    setupResizablePanels();
    setupMenuDropdown();
    setupThemeSync();
    setupValidationModal();

    // Check ArangoDB status on init and every 5 seconds
    checkArangoStatus();
    setInterval(checkArangoStatus, 5000);

    console.log('Fragment Review Application initialized');
}

// Load state from localStorage
function loadStateFromLocalStorage() {
    const savedState = localStorage.getItem('fragmentReviewState');
    if (savedState) {
        try {
            const parsed = JSON.parse(savedState);
            state.selectedFile = parsed.selectedFile;
            state.selectedFragmentId = parsed.selectedFragmentId;
        } catch (e) {
            console.error('Error loading state from localStorage:', e);
        }
    }
}

// Save state to localStorage
function saveStateToLocalStorage() {
    const stateToSave = {
        selectedFile: state.selectedFile,
        selectedFragmentId: state.selectedFragmentId
    };
    localStorage.setItem('fragmentReviewState', JSON.stringify(stateToSave));
}

// Fetch and populate file list from API
async function fetchAndPopulateFileList() {
    const fileList = document.getElementById('file-list');
    fileList.innerHTML = '<div class="panel-item">Loading...</div>';
    
    try {
        const response = await fetch('/api/files');
        if (!response.ok) throw new Error('Failed to fetch files');
        
        const nikayaGroups = await response.json();
        fileList.innerHTML = '';
        
        if (nikayaGroups.length === 0) {
            fileList.innerHTML = '<div class="panel-item">No files found</div>';
        } else {
            nikayaGroups.forEach(group => {
                // Create nikaya header
                const header = document.createElement('div');
                header.className = 'panel-item has-background-grey-lighter has-text-weight-bold';
                header.style.borderBottom = '2px solid #dbdbdb';
                header.style.cursor = 'default';
                header.textContent = group.display_name;
                fileList.appendChild(header);
                
                // Add files under this nikaya
                group.files.forEach(file => {
                    const item = document.createElement('div');
                    item.className = 'panel-item file-item';
                    item.style.paddingLeft = '1.5rem'; // Indent files under nikaya
                    item.style.display = 'flex';
                    item.style.justifyContent = 'space-between';
                    item.style.alignItems = 'center';

                    // Create text content span
                    const textSpan = document.createElement('span');
                    textSpan.className = 'file-text';
                    textSpan.textContent = `${file.filename} (${file.fragment_count})`;
                    textSpan.style.flexGrow = '1';
                    textSpan.style.cursor = 'pointer';
                    textSpan.onclick = () => selectFile(file.filename);
                    item.appendChild(textSpan);

                    // Create reparse button
                    const reparseBtn = document.createElement('button');
                    reparseBtn.className = 'button is-small is-info reparse-btn';
                    reparseBtn.title = 'Reparse this file using current DB as reference';
                    reparseBtn.innerHTML = '&#x21bb;'; // Clockwise arrow (↻)
                    reparseBtn.style.marginLeft = '0.5rem';
                    reparseBtn.style.minWidth = '28px';
                    reparseBtn.onclick = (e) => {
                        e.stopPropagation(); // Prevent file selection
                        reparseFile(file.filename);
                    };
                    item.appendChild(reparseBtn);

                    item.dataset.filename = file.filename;
                    fileList.appendChild(item);
                });
            });
        }
        
        // Restore state after loading files
        await restoreStateAfterInit();
    } catch (error) {
        console.error('Error fetching files:', error);
        fileList.innerHTML = '<div class="panel-item has-text-danger">Error loading files</div>';
    }
}

// Select a file and populate fragments
async function selectFile(filename) {
    state.selectedFile = filename;
    saveStateToLocalStorage();
    
    // Update UI: highlight selected file
    document.querySelectorAll('#file-list .panel-item').forEach(item => {
        item.classList.toggle('is-active', item.dataset.filename === filename);
    });
    
    // Scroll selected file into view
    const selectedItem = document.querySelector(`#file-list .panel-item[data-filename="${filename}"]`);
    if (selectedItem) {
        selectedItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
    
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
                ${createReviewDropdownHtml(fragment.id, fragment.frag_review)}
                <span class="tag">${fragment.frag_idx}</span>
                <span class="tag is-${statusColor}">${fragment.frag_type}</span>
                <span class="tag">${fragment.cst_code}</span>
                <span class="tag">${fragment.sc_code}</span>
            `;
            item.dataset.fragmentId = fragment.id;
            item.dataset.fragIdx = fragment.frag_idx;
            item.dataset.fragReview = fragment.frag_review || '';
            item.onclick = (e) => {
                // Don't select fragment if clicking on the dropdown
                if (e.target.classList.contains('review-dropdown')) return;
                selectFragment(fragment.id);
            };
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
        'needs-review': 'danger',
        'moved': 'info'
    };
    return colorMap[status] || 'light';
}

// Review status options with their labels
const reviewStatusOptions = [
    { value: '', label: 'U', title: 'unchecked' },
    { value: 'in-progress', label: 'I', title: 'in-progress' },
    { value: 'checked', label: 'C', title: 'checked' },
    { value: 'needs-review', label: 'N', title: 'needs-review' },
    { value: 'moved', label: 'M', title: 'moved' }
];

// Get first letter label for review status
function getReviewStatusLabel(status) {
    const option = reviewStatusOptions.find(o => o.value === (status || ''));
    return option ? option.label : 'U';
}

// Generate review status dropdown HTML for fragment list
function createReviewDropdownHtml(fragmentId, currentStatus) {
    const options = reviewStatusOptions.map(opt => {
        const selected = (opt.value === (currentStatus || '')) ? 'selected' : '';
        return `<option value="${opt.value}" ${selected} title="${opt.title}">${opt.label}</option>`;
    }).join('');

    return `<select class="review-dropdown" data-fragment-id="${fragmentId}" title="Review status">${options}</select>`;
}

// Update review status from fragment list dropdown
async function updateReviewStatusFromList(fragmentId, newStatus) {
    try {
        const response = await fetch(`/api/fragments/${fragmentId}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ frag_review: newStatus })
        });

        if (!response.ok) throw new Error('Failed to update review status');

        const updatedFragment = await response.json();

        // Update state
        const stateIndex = state.fragments.findIndex(f => f.id === fragmentId);
        if (stateIndex !== -1) {
            state.fragments[stateIndex] = updatedFragment;
        }

        // Update the fragment item's data attribute and status color
        const fragmentItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${fragmentId}"]`);
        if (fragmentItem) {
            fragmentItem.dataset.fragReview = updatedFragment.frag_review || '';

            // Update the frag_type tag color (3rd child element)
            const fragTypeTag = fragmentItem.querySelector('.tag:nth-child(3)');
            if (fragTypeTag) {
                // Remove old color classes
                fragTypeTag.classList.remove('is-light', 'is-warning', 'is-success', 'is-danger', 'is-info');
                // Add new color class
                const newColor = getStatusColor(updatedFragment.frag_review);
                fragTypeTag.classList.add(`is-${newColor}`);
            }
        }

        // If this is the currently selected fragment, update the metadata panel dropdown too
        if (state.selectedFragmentId === fragmentId) {
            document.getElementById('frag_review').value = updatedFragment.frag_review || '';
        }
    } catch (error) {
        console.error('Error updating review status:', error);
        alert('Failed to update review status');
    }
}

// Auto-fill sc_sutta field based on sc_code using cached Pali titles from ArangoDB
function autoFillScSutta(scCode) {
    if (!scCode || !window.paliTitlesCache) return;

    // Extract the base sutta code (e.g., "dn1" from "dn1:1.1" or "sn1.1" from "sn1.1:1")
    // The ArangoDB titles use the base code without segment numbers
    const baseCode = scCode.split(':')[0];

    // Look up the title in the cache
    const title = window.paliTitlesCache[baseCode];

    if (title) {
        document.getElementById('sc_sutta').value = title;
        document.getElementById('sc_sutta_inline').value = title;
    }
}

// Update a single fragment item in the list without reloading the entire list
// This function now expects the fragment data to be passed in, avoiding race conditions
function updateFragmentItemInList(fragmentId, fragmentData = null) {
    try {
        // If no data provided, fetch it (fallback for backward compatibility)
        if (!fragmentData) {
            fetchFragmentAndUpdateList(fragmentId);
            return;
        }
        
        const fragment = fragmentData;

        // Find the fragment item in the DOM
        const fragmentItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${fragmentId}"]`);
        if (!fragmentItem) return;
        
        // Update the fragment in the state array
        const stateIndex = state.fragments.findIndex(f => f.id === fragmentId);
        if (stateIndex !== -1) {
            state.fragments[stateIndex] = fragment;
        }
        
        // Update the DOM content
        const statusColor = getStatusColor(fragment.frag_review);
        fragmentItem.innerHTML = `
            ${createReviewDropdownHtml(fragment.id, fragment.frag_review)}
            <span class="tag">${fragment.frag_idx}</span>
            <span class="tag is-${statusColor}">${fragment.frag_type}</span>
            <span class="tag">${fragment.cst_code}</span>
            <span class="tag">${fragment.sc_code}</span>
        `;
        fragmentItem.dataset.fragReview = fragment.frag_review || '';

    } catch (error) {
        console.error('Error updating fragment item:', error);
    }
}

// Fallback function to fetch fragment data and update list (for backward compatibility)
async function fetchFragmentAndUpdateList(fragmentId) {
    try {
        const response = await fetch(`/api/fragments/${fragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        
        const fragment = await response.json();
        updateFragmentItemInList(fragmentId, fragment);
    } catch (error) {
        console.error('Error fetching fragment for update:', error);
    }
}

// Remove a fragment item from the list without reloading the entire list
function removeFragmentItemFromList(fragmentId) {
    // Find and remove the fragment item from the DOM
    const fragmentItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${fragmentId}"]`);
    if (fragmentItem) {
        fragmentItem.remove();
    }
    
    // Remove from state array
    const stateIndex = state.fragments.findIndex(f => f.id === fragmentId);
    if (stateIndex !== -1) {
        state.fragments.splice(stateIndex, 1);
    }
}

// Add a new fragment item to the list without reloading the entire list
async function addFragmentItemToList(fragmentId, direction) {
    try {
        const response = await fetch(`/api/fragments/${fragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        
        const fragment = await response.json();
        
        // Find the current fragment item
        const currentItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${state.selectedFragmentId}"]`);
        if (!currentItem) return;
        
        // Create new fragment item
        const newItem = document.createElement('div');
        newItem.className = 'panel-item';
        newItem.dataset.fragmentId = fragment.id;
        newItem.onclick = () => selectFragment(fragment.id);
        
        const statusColor = getStatusColor(fragment.frag_review);
        newItem.innerHTML = `
            <span class="tag">${fragment.frag_idx}</span>
            <span class="tag is-${statusColor}">${fragment.frag_type}</span>
            <span class="tag">${fragment.cst_code}</span>
            <span class="tag">${fragment.sc_code}</span>
        `;
        
        // Insert in the correct position
        if (direction === 'prev') {
            currentItem.parentNode.insertBefore(newItem, currentItem);
        } else {
            currentItem.parentNode.insertBefore(newItem, currentItem.nextSibling);
        }
        
        // Add to state array
        const currentIndex = state.fragments.findIndex(f => f.id === state.selectedFragmentId);
        if (currentIndex !== -1) {
            if (direction === 'prev') {
                state.fragments.splice(currentIndex, 0, fragment);
            } else {
                state.fragments.splice(currentIndex + 1, 0, fragment);
            }
        }
        
    } catch (error) {
        console.error('Error adding fragment item:', error);
    }
}

// Select a fragment and show details
async function selectFragment(fragmentId) {
    state.selectedFragmentId = fragmentId;
    saveStateToLocalStorage();
    
    // Update UI: highlight selected fragment
    document.querySelectorAll('#fragment-list .panel-item').forEach(item => {
        item.classList.toggle('is-active', parseInt(item.dataset.fragmentId) === fragmentId);
    });
    
    // Scroll selected fragment into view
    const selectedItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${fragmentId}"]`);
    if (selectedItem) {
        selectedItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
    
    await fetchAndDisplayFragmentDetails(fragmentId);
}

// Fetch and display fragment details from API
async function fetchAndDisplayFragmentDetails(fragmentId) {
    try {
        const response = await fetch(`/api/fragments/${fragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        
        const detail = await response.json();
        
        // Update metadata fields
        document.getElementById('id').value = detail.id;
        document.getElementById('cst_file').value = detail.cst_file;
        document.getElementById('frag_review').value = detail.frag_review || '';
        document.getElementById('nikaya').value = detail.nikaya;
        document.getElementById('cst_code').value = detail.cst_code || '';
        document.getElementById('cst_vagga').value = detail.cst_vagga || '';
        document.getElementById('cst_sutta').value = detail.cst_sutta || '';
        document.getElementById('cst_paranum').value = detail.cst_paranum || '';
        document.getElementById('sc_code').value = detail.sc_code || '';
        document.getElementById('sc_sutta').value = detail.sc_sutta || '';
        document.getElementById('sc_sutta_inline').value = detail.sc_sutta || '';
        document.getElementById('start_line').value = detail.start_line;
        document.getElementById('start_char').value = detail.start_char;
        document.getElementById('end_line').value = detail.end_line;
        document.getElementById('end_char').value = detail.end_char;
        document.getElementById('group_levels').value = detail.group_levels;
        
        // Update text areas
        const prevTextarea = document.getElementById('prev-content');
        const currentTopTextarea = document.getElementById('current-content-top');
        const currentBottomTextarea = document.getElementById('current-content-bottom');
        const nextTextarea = document.getElementById('next-content');
        
        prevTextarea.value = detail.prev_fragment ? detail.prev_fragment.content_xml : '';
        
        // Both textareas show the complete content_xml
        const currentContent = detail.content_xml || '';
        currentTopTextarea.value = currentContent;
        currentBottomTextarea.value = currentContent;
        
        // Scroll top textarea to top and bottom textarea to bottom
        setTimeout(() => {
            currentTopTextarea.scrollTop = 0;
            // Scroll to bottom, but handle case where content is shorter than textarea height
            const maxScrollTop = Math.max(0, currentBottomTextarea.scrollHeight - currentBottomTextarea.clientHeight);
            currentBottomTextarea.scrollTop = maxScrollTop;
        }, 0);
        
        nextTextarea.value = detail.next_fragment ? detail.next_fragment.content_xml : '';
        
        // Update previous fragment metadata fields
        if (detail.prev_fragment) {
            document.getElementById('prev_cst_code').value = detail.prev_fragment.cst_code || '';
            document.getElementById('prev_sc_code').value = detail.prev_fragment.sc_code || '';
            document.getElementById('prev_cst_vagga').value = detail.prev_fragment.cst_vagga || '';
            document.getElementById('prev_cst_sutta').value = detail.prev_fragment.cst_sutta || '';
            document.getElementById('prev_sc_sutta').value = detail.prev_fragment.sc_sutta || '';
        } else {
            document.getElementById('prev_cst_code').value = '';
            document.getElementById('prev_sc_code').value = '';
            document.getElementById('prev_cst_vagga').value = '';
            document.getElementById('prev_cst_sutta').value = '';
            document.getElementById('prev_sc_sutta').value = '';
        }

        // Update next fragment metadata fields
        if (detail.next_fragment) {
            document.getElementById('next_cst_code').value = detail.next_fragment.cst_code || '';
            document.getElementById('next_sc_code').value = detail.next_fragment.sc_code || '';
            document.getElementById('next_cst_vagga').value = detail.next_fragment.cst_vagga || '';
            document.getElementById('next_cst_sutta').value = detail.next_fragment.cst_sutta || '';
            document.getElementById('next_sc_sutta').value = detail.next_fragment.sc_sutta || '';
        } else {
            document.getElementById('next_cst_code').value = '';
            document.getElementById('next_sc_code').value = '';
            document.getElementById('next_cst_vagga').value = '';
            document.getElementById('next_cst_sutta').value = '';
            document.getElementById('next_sc_sutta').value = '';
        }
        
        // Scroll previous fragment textarea to bottom
        if (detail.prev_fragment) {
            // Use setTimeout to ensure the textarea has been updated
            setTimeout(() => {
                prevTextarea.scrollTop = prevTextarea.scrollHeight;
            }, 0);
        }
        
        // Enable/disable controls based on position and fragment type
        const hasPrev = detail.prev_fragment !== null;
        const hasNext = detail.next_fragment !== null;
        
        // Move buttons should only be enabled if the current fragment is Sutta
        // and there's an adjacent Sutta fragment to merge with
        const currentIsSutta = detail.frag_type === 'Sutta';
        const currentIsHeader = detail.frag_type === 'Header';
        const prevIsSutta = detail.prev_fragment && detail.prev_fragment.frag_type === 'Sutta';
        const nextIsSutta = detail.next_fragment && detail.next_fragment.frag_type === 'Sutta';
        const prevIsHeader = detail.prev_fragment && detail.prev_fragment.frag_type === 'Header';
        const nextIsHeader = detail.next_fragment && detail.next_fragment.frag_type === 'Header';
        
        let el;
        el = document.getElementById('move-to-prev');
        if (el) {
            // Enable move-to-prev button only if current is Sutta and has previous Sutta to move content to
            el.disabled = !(currentIsSutta && prevIsSutta);
        }

        el = document.getElementById('move-to-next');
        if (el) {
            // Enable move-to-next button only if current is Sutta and has next Sutta to move content to
            el.disabled = !(currentIsSutta && nextIsSutta);
        }

        el = document.getElementById('create-prev-btn');
        if (el) {
            // Create new prev button: disabled if current is the first Header (no prev Header exists)
            el.disabled = currentIsHeader && !prevIsHeader;
        }

        el = document.getElementById('create-next-btn');
        if (el) {
            // Create new next button: disabled if current is the last Header (no next Header exists)
            el.disabled = currentIsHeader && !nextIsHeader;
        }

        // Boundary adjustment buttons enabled if there's a prev/next fragment
        document.querySelectorAll('[id^="prev-"]').forEach(btn => btn.disabled = !hasPrev);
        document.querySelectorAll('[id^="next-"]').forEach(btn => btn.disabled = !hasNext);
        
    } catch (error) {
        console.error('Error fetching fragment details:', error);
        alert('Error loading fragment details');
    }
}

// Clear fragment details
function clearFragmentDetails() {
    document.getElementById('id').value = '';
    document.getElementById('cst_file').value = '';
    document.getElementById('frag_review').value = '';
    document.getElementById('nikaya').value = '';
    document.getElementById('cst_code').value = '';
    document.getElementById('sc_code').value = '';
    document.getElementById('cst_vagga').value = '';
    document.getElementById('cst_sutta').value = '';
    document.getElementById('cst_paranum').value = '';
    document.getElementById('sc_sutta').value = '';
    document.getElementById('sc_sutta_inline').value = '';
    document.getElementById('start_line').value = '';
    document.getElementById('start_char').value = '';
    document.getElementById('end_line').value = '';
    document.getElementById('end_char').value = '';
    document.getElementById('group_levels').value = '';
    
    // Clear previous fragment fields
    document.getElementById('prev_cst_code').value = '';
    document.getElementById('prev_sc_code').value = '';
    document.getElementById('prev_cst_vagga').value = '';
    document.getElementById('prev_cst_sutta').value = '';
    document.getElementById('prev_sc_sutta').value = '';

    // Clear next fragment fields
    document.getElementById('next_cst_code').value = '';
    document.getElementById('next_sc_code').value = '';
    document.getElementById('next_cst_vagga').value = '';
    document.getElementById('next_cst_sutta').value = '';
    document.getElementById('next_sc_sutta').value = '';
    
    document.getElementById('prev-content').value = '';
    document.getElementById('current-content-top').value = '';
    document.getElementById('current-content-bottom').value = '';
    document.getElementById('next-content').value = '';
}

// Update fragment metadata (auto-save on blur)
async function updateFragmentMetadata() {
    if (!state.selectedFragmentId) return;
    
    const metadata = {
        frag_review: document.getElementById('frag_review').value || '',
        cst_code: document.getElementById('cst_code').value || '',
        sc_code: document.getElementById('sc_code').value || '',
        cst_vagga: document.getElementById('cst_vagga').value || '',
        cst_sutta: document.getElementById('cst_sutta').value || '',
        cst_paranum: document.getElementById('cst_paranum').value || '',
        sc_sutta: document.getElementById('sc_sutta').value || '',
    };
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(metadata)
        });
        
        if (!response.ok) throw new Error('Failed to update metadata');
        
        // Use the returned fragment data to update the UI (no race condition)
        const updatedFragment = await response.json();
        updateFragmentItemInList(state.selectedFragmentId, updatedFragment);
    } catch (error) {
        console.error('Error updating metadata:', error);
        alert('Failed to save metadata changes');
    }
}

// Update adjacent fragment metadata (previous or next)
async function updateAdjacentFragmentMetadata(direction) {
    if (!state.selectedFragmentId) return;
    
    // Get current fragment details to find adjacent fragment ID
    const detail = await getCurrentFragmentDetail();
    if (!detail) return;
    
    let adjacentFragment;
    if (direction === 'prev') {
        adjacentFragment = detail.prev_fragment;
    } else {
        adjacentFragment = detail.next_fragment;
    }
    
    if (!adjacentFragment) {
        console.log(`No ${direction} fragment found to update`);
        return;
    }
    
    const metadata = {
        cst_code: document.getElementById(`${direction}_cst_code`).value || '',
        sc_code: document.getElementById(`${direction}_sc_code`).value || '',
        cst_vagga: document.getElementById(`${direction}_cst_vagga`).value || '',
        cst_sutta: document.getElementById(`${direction}_cst_sutta`).value || '',
    };
    
    try {
        const response = await fetch(`/api/fragments/${adjacentFragment.id}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(metadata)
        });
        
        if (!response.ok) throw new Error('Failed to update adjacent fragment metadata');
        
        // Use the returned fragment data to update the UI (no race condition)
        const updatedFragment = await response.json();
        updateFragmentItemInList(adjacentFragment.id, updatedFragment);
    } catch (error) {
        console.error('Error updating adjacent fragment metadata:', error);
        alert('Failed to save adjacent fragment metadata changes');
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

        // Refresh the current fragment view to update boundary data and adjacent fragment metadata
        await fetchAndDisplayFragmentDetails(state.selectedFragmentId);

        console.log('Boundary adjusted:', result.message);
    } catch (error) {
        console.error('Error adjusting boundary:', error);
        alert('Failed to adjust boundary');
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

// Create new fragment before or after current fragment
async function createNewFragment(direction) {
    if (!state.selectedFragmentId) return;
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}/create`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ direction })
        });
        
        if (!response.ok) throw new Error('Failed to create new fragment');
        
        const result = await response.json();
        
        // Add the new fragment to the DOM and state
        await addFragmentItemToList(result.new_fragment_id, direction);
        
        // Select the newly created fragment
        await selectFragment(result.new_fragment_id);
        
        console.log('New fragment created successfully');
    } catch (error) {
        console.error('Error creating new fragment:', error);
        alert('Failed to create new fragment');
    }
}

// Move fragment content to adjacent fragment (prev or next)
async function moveFragmentTo(direction) {
    if (!state.selectedFragmentId) {
        alert('No fragment selected');
        return;
    }
    
    try {
        // Get current fragment detail to retrieve cst_file
        const currentFragment = await getCurrentFragmentDetail();
        if (!currentFragment) {
            alert('Failed to get current fragment details');
            return;
        }
        
        // Construct request body
        const requestBody = {
            frag_idx: currentFragment.frag_idx,
            xml_file: currentFragment.cst_file,
            direction: direction
        };
        
        // Make POST request to move endpoint
        const response = await fetch('/api/fragments/move', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(requestBody)
        });
        
        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(errorText || 'Failed to move fragment');
        }
        
        const result = await response.json();
        
        // Update DOM for current fragment (now moved/empty)
        updateFragmentItemInList(result.current_fragment.id, result.current_fragment);
        
        // Update DOM for target fragment (now has merged content)
        updateFragmentItemInList(result.target_fragment.id, result.target_fragment);
        
        // Refresh the current fragment detail view to show updated boundaries and content
        await fetchAndDisplayFragmentDetails(state.selectedFragmentId);
        
        console.log(`Fragment content moved to ${direction} successfully`);
    } catch (error) {
        console.error('Error moving fragment:', error);
        alert(`Failed to move fragment: ${error.message}`);
    }
}

// Setup event listeners for buttons
function setupEventListeners() {
    // Event delegation for review status dropdowns in fragment list
    document.getElementById('fragment-list').addEventListener('change', function(e) {
        if (e.target.classList.contains('review-dropdown')) {
            const fragmentId = parseInt(e.target.dataset.fragmentId);
            const newStatus = e.target.value;
            updateReviewStatusFromList(fragmentId, newStatus);
        }
    });

    // Auto-save metadata on input change with debouncing
    let updateTimeout;
    function debouncedUpdateFragmentMetadata() {
        clearTimeout(updateTimeout);
        updateTimeout = setTimeout(() => {
            updateFragmentMetadata();
        }, 300);
    }
    
    // Use both input and change events to catch all scenarios
    function setupInputHandlers(elementId) {
        const element = document.getElementById(elementId);
        element.addEventListener('input', function() {
            debouncedUpdateFragmentMetadata();
        });
        element.addEventListener('change', updateFragmentMetadata); // update on blur/change
    }
    
    setupInputHandlers('frag_review');
    setupInputHandlers('cst_code');
    setupInputHandlers('sc_code');
    setupInputHandlers('cst_vagga');
    setupInputHandlers('cst_sutta');
    setupInputHandlers('cst_paranum');
    setupInputHandlers('sc_sutta');

    // Auto-fill sc_sutta when sc_code changes
    document.getElementById('sc_code').addEventListener('input', function() {
        autoFillScSutta(this.value);
    });

    // Sync sc_sutta and sc_sutta_inline fields
    document.getElementById('sc_sutta').addEventListener('input', function() {
        document.getElementById('sc_sutta_inline').value = this.value;
    });
    document.getElementById('sc_sutta_inline').addEventListener('input', function() {
        document.getElementById('sc_sutta').value = this.value;
        debouncedUpdateFragmentMetadata();
    });
    document.getElementById('sc_sutta_inline').addEventListener('change', updateFragmentMetadata);
    
    // Previous fragment metadata input handlers with debouncing
    let prevUpdateTimeout;
    function debouncedUpdatePrevFragmentMetadata() {
        clearTimeout(prevUpdateTimeout);
        prevUpdateTimeout = setTimeout(() => updateAdjacentFragmentMetadata('prev'), 300);
    }
    
    function setupPrevHandlers(elementId) {
        const element = document.getElementById(`prev_${elementId}`);
        element.addEventListener('input', function() {
            debouncedUpdatePrevFragmentMetadata();
        });
        element.addEventListener('change', () => updateAdjacentFragmentMetadata('prev'));
    }
    
    setupPrevHandlers('cst_code');
    setupPrevHandlers('sc_code');
    setupPrevHandlers('cst_vagga');
    setupPrevHandlers('cst_sutta');
    
    // Next fragment metadata input handlers with debouncing
    let nextUpdateTimeout;
    function debouncedUpdateNextFragmentMetadata() {
        clearTimeout(nextUpdateTimeout);
        nextUpdateTimeout = setTimeout(() => updateAdjacentFragmentMetadata('next'), 300);
    }
    
    function setupNextHandlers(elementId) {
        const element = document.getElementById(`next_${elementId}`);
        element.addEventListener('input', function() {
            debouncedUpdateNextFragmentMetadata();
        });
        element.addEventListener('change', () => updateAdjacentFragmentMetadata('next'));
    }
    
    setupNextHandlers('cst_code');
    setupNextHandlers('sc_code');
    setupNextHandlers('cst_vagga');
    setupNextHandlers('cst_sutta');
    
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

    let el;
    // Move buttons with confirmation
    el = document.getElementById('move-to-prev');
    if (el) {
        el.onclick = () => {
            showConfirmModal("Move the current fragment's content to the PREVIOUS fragment?", () => {
                moveFragmentTo('prev');
            });
        };
    }

    el = document.getElementById('move-to-next');
    if (el) {
        el.onclick = () => {
            showConfirmModal("Move the current fragment's content to the NEXT fragment?", () => {
                moveFragmentTo('next');
            });
        };
    }
    
    // Create new fragment buttons
    el = document.getElementById('create-prev-btn');
    if (el) {
        el.onclick = () => createNewFragment('prev');
    }
    el = document.getElementById('create-next-btn');
    if (el) {
        el.onclick = () => createNewFragment('next');
    }

    // Modal controls
    document.getElementById('modal-close').onclick = closeModal;
    document.getElementById('modal-cancel').onclick = closeModal;
    
    // Menu buttons
    document.getElementById('menu-settings').onclick = (e) => {
        e.preventDefault();
        closeMenuDropdown();
        openSettingsModal();
    };
    
    document.getElementById('menu-regenerate').onclick = (e) => {
        e.preventDefault();
        closeMenuDropdown();
        openRegenerateModal();
    };
    
    // Settings modal controls
    document.getElementById('settings-modal-close').onclick = closeSettingsModal;
    document.getElementById('settings-cancel').onclick = closeSettingsModal;
    document.getElementById('settings-save').onclick = saveSettings;
    
    // Theme change handler
    document.getElementById('settings-color-theme').onchange = (e) => {
        setTheme(e.target.value);
    };
    
    // Regenerate modal controls
    document.getElementById('regenerate-modal-close').onclick = closeRegenerateModal;
    document.getElementById('regenerate-close').onclick = closeRegenerateModal;
    document.getElementById('regenerate-with-reference').onclick = () => startRegeneration(true);
    document.getElementById('regenerate-new-replace').onclick = () => startRegeneration(false);
    document.getElementById('copy-output-btn').onclick = copyOutputToClipboard;
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
        if (newLeftWidth >= 250 && newLeftWidth <= 800) {
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

// Setup menu dropdown toggle
function setupMenuDropdown() {
    const dropdown = document.getElementById('menu-dropdown');
    const trigger = dropdown.querySelector('.dropdown-trigger button');
    
    trigger.addEventListener('click', (e) => {
        e.stopPropagation();
        dropdown.classList.toggle('is-active');
    });
    
    // Close dropdown when clicking outside
    document.addEventListener('click', (e) => {
        if (!dropdown.contains(e.target)) {
            dropdown.classList.remove('is-active');
        }
    });
}

// Close menu dropdown
function closeMenuDropdown() {
    document.getElementById('menu-dropdown').classList.remove('is-active');
}

// Setup theme synchronization with server settings
async function setupThemeSync() {
    try {
        const response = await fetch('/api/settings');
        if (response.ok) {
            const settings = await response.json();
            const serverTheme = settings.color_theme || 'system';
            
            // Only apply server theme if user hasn't set a local preference
            if (!localStorage.getItem('colorTheme')) {
                setTheme(serverTheme);
            }
        }
    } catch (error) {
        console.error('Error loading settings for theme sync:', error);
    }
}

// Open Settings modal
async function openSettingsModal() {
    try {
        const response = await fetch('/api/settings');
        if (response.ok) {
            const settings = await response.json();
            document.getElementById('settings-db-path').value = settings.db_path || '';
            document.getElementById('settings-port').value = settings.port || 8000;
            document.getElementById('settings-color-theme').value = settings.color_theme || 'system';
            document.getElementById('settings-xml-dir').value = settings.xml_dir || '';
            document.getElementById('settings-xml-filenames').value = (settings.xml_filenames || []).join('\n');
            document.getElementById('settings-parser-path').value = settings.xml_parser_binary_path || 'target/debug/tipitaka_xml_parser';
            document.getElementById('settings-new-db-path').value = settings.new_fragments_db_path || '';
            document.getElementById('settings-ref-db-path').value = settings.reference_fragments_db_path || '';
            document.getElementById('settings-new-tsv-path').value = settings.new_fragments_tsv_path || '';
            document.getElementById('settings-ref-tsv-path').value = settings.reference_fragments_tsv_path || '';
        } else {
            console.error('Failed to load settings');
        }
    } catch (error) {
        console.error('Error loading settings:', error);
    }
    
    document.getElementById('settings-modal').classList.add('is-active');
}

// Close Settings modal
function closeSettingsModal() {
    document.getElementById('settings-modal').classList.remove('is-active');
}

// Save settings
async function saveSettings() {
    const settings = {
        db_path: document.getElementById('settings-db-path').value,
        port: parseInt(document.getElementById('settings-port').value) || 8000,
        color_theme: document.getElementById('settings-color-theme').value,
        xml_dir: document.getElementById('settings-xml-dir').value,
        xml_filenames: document.getElementById('settings-xml-filenames').value
            .split('\n')
            .map(line => line.trim())
            .filter(line => line.length > 0),
        xml_parser_binary_path: document.getElementById('settings-parser-path').value || null,
        new_fragments_db_path: document.getElementById('settings-new-db-path').value || null,
        reference_fragments_db_path: document.getElementById('settings-ref-db-path').value || null,
        new_fragments_tsv_path: document.getElementById('settings-new-tsv-path').value || null,
        reference_fragments_tsv_path: document.getElementById('settings-ref-tsv-path').value || null
    };
    
    try {
        const response = await fetch('/api/settings', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(settings)
        });
        
        if (response.ok) {
            console.log('Settings saved successfully');
            closeSettingsModal();
            
            // Show success notification
            alert('Settings saved successfully. Some changes may require a server restart.');
        } else {
            console.error('Failed to save settings');
            alert('Failed to save settings');
        }
    } catch (error) {
        console.error('Error saving settings:', error);
        alert('Error saving settings');
    }
}

// Open Regenerate modal
function openRegenerateModal() {
    // Reset modal state
    document.getElementById('regenerate-start-message').style.display = 'block';
    document.getElementById('regenerate-status').style.display = 'none';
    document.getElementById('regenerate-output-container').style.display = 'none';
    document.getElementById('regenerate-output').textContent = '';
    document.getElementById('regenerate-with-reference').disabled = false;
    document.getElementById('regenerate-new-replace').disabled = false;

    // Reset modal title to default
    document.querySelector('#regenerate-modal .modal-card-title').textContent = 'Regenerate Fragments Database';

    document.getElementById('regenerate-modal').classList.add('is-active');
}

// Close Regenerate modal
function closeRegenerateModal() {
    document.getElementById('regenerate-modal').classList.remove('is-active');
    
    // If DB was replaced, reload the page now
    if (needsReloadAfterClose) {
        needsReloadAfterClose = false;
        setTimeout(() => {
            window.location.reload();
        }, 100);
    }
}

// Global flag to track if reload is needed
let needsReloadAfterClose = false;

// Start regeneration process
async function startRegeneration(useReferenceDb) {
    // Hide start message, show status
    document.getElementById('regenerate-start-message').style.display = 'none';
    document.getElementById('regenerate-status').style.display = 'block';
    document.getElementById('regenerate-output-container').style.display = 'block';
    document.getElementById('regenerate-with-reference').disabled = true;
    document.getElementById('regenerate-new-replace').disabled = true;
    
    // Reset reload flag
    needsReloadAfterClose = false;
    
    // Update status
    updateRegenerateStatus('Running...', 'is-info');
    
    try {
        const response = await fetch('/api/regenerate', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ use_reference_db: useReferenceDb })
        });
        
        // Always try to parse as JSON since we always return JSON now
        const result = await response.json();
        
        // Display output
        document.getElementById('regenerate-output').textContent = result.output;
        
        // Update status based on success
        if (result.success) {
            if (result.db_replaced) {
                updateRegenerateStatus('Completed successfully! Close modal to reload UI.', 'is-success');
                needsReloadAfterClose = true;
            } else {
                updateRegenerateStatus('Completed successfully!', 'is-success');
            }
        } else {
            // Check if this is a configuration error (output starts with ERROR:)
            if (result.output.startsWith('ERROR:')) {
                updateRegenerateStatus('Configuration Error', 'is-danger');
            } else {
                updateRegenerateStatus('Completed with errors', 'is-warning');
            }
        }
        
    } catch (error) {
        console.error('Regeneration failed:', error);
        updateRegenerateStatus('Failed: ' + error.message, 'is-danger');
        
        // Only append if there's no existing output
        const outputEl = document.getElementById('regenerate-output');
        if (!outputEl.textContent) {
            outputEl.textContent = 'Error: ' + error.message;
        }
    }
}

// Reparse a single file using current DB as reference
async function reparseFile(cstFile) {
    // Show confirmation dialog
    if (!confirm(`Reparse file "${cstFile}"?\n\nThis will re-parse the XML file using the current database as reference, preserving checked fragment overrides and review status.`)) {
        return;
    }

    // Show regenerate modal with reparse context
    const modal = document.getElementById('regenerate-modal');
    modal.classList.add('is-active');

    // Update modal title for reparse context
    modal.querySelector('.modal-card-title').textContent = `Reparsing: ${cstFile}`;

    // Hide start message, show status
    document.getElementById('regenerate-start-message').style.display = 'none';
    document.getElementById('regenerate-status').style.display = 'block';
    document.getElementById('regenerate-output-container').style.display = 'block';
    document.getElementById('regenerate-output').textContent = '';

    // Disable regenerate buttons during reparse
    document.getElementById('regenerate-with-reference').disabled = true;
    document.getElementById('regenerate-new-replace').disabled = true;

    // Set flag to refresh after close
    needsReloadAfterClose = false;

    // Update status
    updateRegenerateStatus('Reparsing file...', 'is-info');

    try {
        const response = await fetch('/api/reparse-file', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ cst_file: cstFile })
        });

        const result = await response.json();

        // Display output
        document.getElementById('regenerate-output').textContent = result.output;

        // Update status based on success
        if (result.success) {
            updateRegenerateStatus(
                `Reparse completed! ${result.fragments_count} fragments, ${result.review_status_restored} review statuses restored.`,
                'is-success'
            );
            needsReloadAfterClose = true;
        } else {
            if (result.output.includes('ERROR:')) {
                updateRegenerateStatus('Reparse failed', 'is-danger');
            } else {
                updateRegenerateStatus('Reparse completed with warnings', 'is-warning');
            }
        }

    } catch (error) {
        console.error('Reparse failed:', error);
        updateRegenerateStatus('Failed: ' + error.message, 'is-danger');

        const outputEl = document.getElementById('regenerate-output');
        if (!outputEl.textContent) {
            outputEl.textContent = 'Error: ' + error.message;
        }
    }
}

// Update regeneration status message
function updateRegenerateStatus(message, colorClass) {
    const statusDiv = document.getElementById('regenerate-status');
    const statusText = document.getElementById('regenerate-status-text');
    
    // Remove all color classes
    statusDiv.classList.remove('is-info', 'is-success', 'is-warning', 'is-danger');
    
    // Add new color class
    statusDiv.classList.add(colorClass);
    
    // Update text
    statusText.textContent = message;
}

// Copy output to clipboard
function copyOutputToClipboard() {
    const output = document.getElementById('regenerate-output').textContent;
    
    navigator.clipboard.writeText(output).then(() => {
        // Change button text temporarily to show success
        const btn = document.getElementById('copy-output-btn');
        const originalText = btn.innerHTML;
        btn.innerHTML = '<span>✓ Copied!</span>';
        btn.classList.add('is-success');
        
        setTimeout(() => {
            btn.innerHTML = originalText;
            btn.classList.remove('is-success');
        }, 2000);
    }).catch(err => {
        console.error('Failed to copy:', err);
        alert('Failed to copy to clipboard');
    });
}

// Restore state after loading file list
async function restoreStateAfterInit() {
    if (state.selectedFile) {
        // Try to select the previously selected file
        const fileItem = document.querySelector(`#file-list .panel-item[data-filename="${state.selectedFile}"]`);
        if (fileItem) {
            await selectFile(state.selectedFile);

            // Try to select the previously selected fragment
            if (state.selectedFragmentId) {
                // Use setTimeout to ensure fragment list is fully loaded
                setTimeout(async () => {
                    const fragmentItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${state.selectedFragmentId}"]`);
                    if (fragmentItem) {
                        await selectFragment(state.selectedFragmentId);
                    }
                }, 100);
            }
        }
    }
}

// ============================================================================
// ArangoDB Status Functions
// ============================================================================

// Check ArangoDB connection status and update indicator
async function checkArangoStatus() {
    const indicator = document.getElementById('arango-status');
    if (!indicator) return;

    try {
        const response = await fetch('/api/arangodb/status');
        if (!response.ok) throw new Error('Failed to check ArangoDB status');

        const data = await response.json();
        const wasConnected = window.arangoConnected;
        window.arangoConnected = data.connected;

        if (data.connected) {
            indicator.classList.remove('disconnected');
            indicator.classList.add('connected');
            indicator.title = 'ArangoDB: Connected';

            // If connection was just restored and cache is empty, fetch titles
            if (!wasConnected && !window.paliTitlesCache) {
                fetchPaliTitles();
            }
        } else {
            indicator.classList.remove('connected');
            indicator.classList.add('disconnected');
            indicator.title = data.error ? `ArangoDB: ${data.error}` : 'ArangoDB: Disconnected';
        }
    } catch (err) {
        console.error('Error checking ArangoDB status:', err);
        indicator.classList.remove('connected');
        indicator.classList.add('disconnected');
        indicator.title = 'ArangoDB: Connection check failed';
        window.arangoConnected = false;
    }
}

// Fetch Pali titles from ArangoDB and populate cache
async function fetchPaliTitles() {
    try {
        const response = await fetch('/api/arangodb/pali-titles');
        if (!response.ok) throw new Error('Failed to fetch Pali titles');

        window.paliTitlesCache = await response.json();
        console.log(`Loaded ${Object.keys(window.paliTitlesCache).length} Pali titles from ArangoDB`);
    } catch (err) {
        console.error('Error fetching Pali titles:', err);
        // Don't set cache to null on error - keep any existing cache
    }
}

// ============================================================================
// Validation Modal Functions
// ============================================================================

// Open the validation modal
function openValidationModal() {
    // Show modal
    document.getElementById('validation-modal').classList.add('is-active');

    // If we have cached results, restore the UI state
    if (validationResults) {
        // Update badges for all check types
        for (const [checkId, result] of Object.entries(validationResults)) {
            const badge = document.getElementById(`badge-${checkId}`);
            if (badge) {
                const errorCount = result.errors.length;
                badge.textContent = errorCount.toString();
                badge.classList.remove('has-errors', 'no-errors');
                badge.classList.add(errorCount > 0 ? 'has-errors' : 'no-errors');
            }
        }

        // Restore selected check type or select first one
        if (selectedCheckType && validationResults[selectedCheckType]) {
            selectCheckType(selectedCheckType);
        } else {
            const firstCheckId = Object.keys(validationResults)[0];
            if (firstCheckId) {
                selectCheckType(firstCheckId);
            }
        }
    } else {
        // No cached results - show initial state
        selectedCheckType = null;
        document.querySelectorAll('.validation-check-item').forEach(item => {
            item.classList.remove('is-active');
        });
        document.querySelectorAll('.check-badge').forEach(badge => {
            badge.textContent = '-';
            badge.classList.remove('has-errors', 'no-errors');
        });
        document.getElementById('validation-results-title').textContent = 'Select a check to view results';
        document.getElementById('validation-results-list').innerHTML = '<div class="validation-placeholder">Run validation to see results</div>';
        document.getElementById('auto-fix-btn').style.display = 'none';
    }
}

// Close the validation modal
function closeValidationModal() {
    document.getElementById('validation-modal').classList.remove('is-active');
}

// Run all validation checks
async function runValidation() {
    const btn = document.getElementById('run-validation-btn');
    const spinner = btn.querySelector('.validation-spinner');
    const btnText = btn.querySelector('span:last-child');

    // Show loading state
    spinner.style.display = 'inline-block';
    btnText.textContent = 'Running...';
    btn.disabled = true;

    try {
        const response = await fetch('/api/validation/run', { method: 'POST' });
        if (!response.ok) throw new Error('Validation request failed');

        const data = await response.json();
        validationResults = data.checks;

        // Update badges for all check types
        for (const [checkId, result] of Object.entries(validationResults)) {
            const badge = document.getElementById(`badge-${checkId}`);
            if (badge) {
                const errorCount = result.errors.length;
                badge.textContent = errorCount.toString();
                badge.classList.remove('has-errors', 'no-errors');
                badge.classList.add(errorCount > 0 ? 'has-errors' : 'no-errors');
            }
        }

        // Select first check type
        const firstCheckId = Object.keys(validationResults)[0];
        if (firstCheckId) {
            selectCheckType(firstCheckId);
        }

    } catch (err) {
        console.error('Error running validation:', err);
        alert('Failed to run validation: ' + err.message);
    } finally {
        // Reset button state
        spinner.style.display = 'none';
        btnText.textContent = 'Run Validation';
        btn.disabled = false;
    }
}

// Select a check type and show its results
function selectCheckType(checkId) {
    selectedCheckType = checkId;

    // Update active state on check items
    document.querySelectorAll('.validation-check-item').forEach(item => {
        item.classList.toggle('is-active', item.dataset.checkId === checkId);
    });

    // Render results
    renderValidationResults(checkId);
}

// Render validation results for a check type
function renderValidationResults(checkId) {
    const resultsList = document.getElementById('validation-results-list');
    const resultsTitle = document.getElementById('validation-results-title');
    const autoFixBtn = document.getElementById('auto-fix-btn');

    if (!validationResults || !validationResults[checkId]) {
        resultsList.innerHTML = '<div class="validation-placeholder">No results available</div>';
        resultsTitle.textContent = 'No results';
        autoFixBtn.style.display = 'none';
        return;
    }

    const result = validationResults[checkId];
    resultsTitle.textContent = `${result.name} (${result.errors.length} issues)`;

    // Show/hide auto-fix button
    autoFixBtn.style.display = (result.auto_fixable && result.auto_fixes.length > 0) ? 'inline-block' : 'none';

    if (result.errors.length === 0) {
        resultsList.innerHTML = '<div class="validation-placeholder" style="color: #48c774;">No issues found</div>';
        return;
    }

    // Build results HTML
    let html = '';
    for (const error of result.errors) {
        html += `
            <div class="validation-result-item">
                <div class="result-info">
                    <span class="result-location">${error.cst_file} #${error.frag_idx}</span>
                    <span class="result-message">${error.message}</span>
                </div>
                <div class="result-actions">
                    <button class="button is-small is-info" onclick="openFragmentFromValidation('${error.cst_file}', ${error.frag_idx})">
                        Open
                    </button>
                </div>
            </div>
        `;
    }
    resultsList.innerHTML = html;
}

// Open a fragment from validation results
async function openFragmentFromValidation(cstFile, fragIdx) {
    // Close validation modal
    closeValidationModal();

    // Select the file
    await selectFile(cstFile);

    // Wait a moment for fragments to load, then find and select the fragment
    setTimeout(() => {
        const fragmentItem = document.querySelector(`#fragment-list .panel-item[data-frag-idx="${fragIdx}"]`);
        if (fragmentItem) {
            const fragmentId = parseInt(fragmentItem.dataset.fragmentId);
            selectFragment(fragmentId);
            // Scroll fragment into view
            fragmentItem.scrollIntoView({ behavior: 'smooth', block: 'center' });
        } else {
            console.warn(`Fragment with frag_idx ${fragIdx} not found in file ${cstFile}`);
        }
    }, 200);
}

// Show auto-fix confirmation dialog
function showAutoFixConfirmation() {
    if (!validationResults || !selectedCheckType) return;

    const result = validationResults[selectedCheckType];
    if (!result.auto_fixes || result.auto_fixes.length === 0) return;

    const changesList = document.getElementById('autofix-changes-list');
    let html = '';

    for (const fix of result.auto_fixes) {
        html += `
            <div class="autofix-change-item">
                <span class="change-file">${fix.cst_file}</span>
                <span class="change-idx">#${fix.frag_idx}</span>
                <span class="change-code">${fix.sc_code}</span>
                <span class="change-arrow">→</span>
                <span class="change-value">${fix.suggested_value}</span>
            </div>
        `;
    }
    changesList.innerHTML = html;

    // Show confirmation modal
    document.getElementById('autofix-confirm-modal').classList.add('is-active');
}

// Close auto-fix confirmation modal
function closeAutoFixModal() {
    document.getElementById('autofix-confirm-modal').classList.remove('is-active');
}

// Apply auto-fixes
async function applyAutoFixes() {
    if (!validationResults || !selectedCheckType) return;

    const result = validationResults[selectedCheckType];
    if (!result.auto_fixes || result.auto_fixes.length === 0) return;

    // Prepare fixes request
    const fixes = result.auto_fixes.map(fix => ({
        fragment_id: fix.fragment_id,
        suggested_value: fix.suggested_value
    }));

    try {
        const response = await fetch('/api/validation/auto-fix/missing-sc-sutta', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ fixes })
        });

        if (!response.ok) throw new Error('Auto-fix request failed');

        const data = await response.json();
        if (data.success) {
            alert(`Successfully updated ${data.updated_count} fragments`);
            // Close confirmation modal and re-run validation
            closeAutoFixModal();
            await runValidation();
        } else {
            throw new Error(data.error || 'Unknown error');
        }
    } catch (err) {
        console.error('Error applying auto-fixes:', err);
        alert('Failed to apply auto-fixes: ' + err.message);
    }
}

// Setup validation modal event listeners
function setupValidationModal() {
    // Menu item click
    document.getElementById('menu-validation').addEventListener('click', (e) => {
        e.preventDefault();
        document.getElementById('menu-dropdown').classList.remove('is-active');
        openValidationModal();
    });

    // Run Validation button
    document.getElementById('run-validation-btn').addEventListener('click', runValidation);

    // Check type selection
    document.querySelectorAll('.validation-check-item').forEach(item => {
        item.addEventListener('click', () => {
            selectCheckType(item.dataset.checkId);
        });
    });

    // Auto-Fix All button
    document.getElementById('auto-fix-btn').addEventListener('click', showAutoFixConfirmation);

    // Close buttons
    document.getElementById('validation-modal-close').addEventListener('click', closeValidationModal);
    document.getElementById('validation-close').addEventListener('click', closeValidationModal);
    document.getElementById('validation-modal').querySelector('.modal-background').addEventListener('click', closeValidationModal);

    // Auto-fix confirmation modal
    document.getElementById('autofix-modal-close').addEventListener('click', closeAutoFixModal);
    document.getElementById('autofix-cancel').addEventListener('click', closeAutoFixModal);
    document.getElementById('autofix-apply').addEventListener('click', applyAutoFixes);
    document.getElementById('autofix-confirm-modal').querySelector('.modal-background').addEventListener('click', closeAutoFixModal);
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
