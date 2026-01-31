// Search functionality
const searchInput = document.getElementById('searchInput');
const tableBody = document.getElementById('tableBody');
const emptyState = document.getElementById('emptyState');
const filterBtns = document.querySelectorAll('.filter-btn');
const sortHeaders = document.querySelectorAll('.sortable');

let currentFilter = 'all';
let currentSort = { column: null, direction: 'asc' };

// Search
searchInput.addEventListener('input', (e) => {
  filterTable();
});

// Filter buttons
filterBtns.forEach((btn) => {
  btn.addEventListener('click', () => {
    filterBtns.forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    currentFilter = btn.dataset.filter;
    filterTable();
  });
});

// Sort functionality
sortHeaders.forEach((header) => {
  header.addEventListener('click', () => {
    const column = header.dataset.sort;

    if (currentSort.column === column) {
      currentSort.direction = currentSort.direction === 'asc' ? 'desc' : 'asc';
    } else {
      currentSort.column = column;
      currentSort.direction = 'asc';
    }

    sortHeaders.forEach((h) => {
      h.classList.remove('sorted-asc', 'sorted-desc');
    });

    header.classList.add(currentSort.direction === 'asc' ? 'sorted-asc' : 'sorted-desc');

    sortTable();
  });
});

function filterTable() {
  const searchTerm = searchInput.value.toLowerCase();
  const rows = Array.from(tableBody.querySelectorAll('tr'));
  let visibleCount = 0;

  rows.forEach((row) => {
    const name = row.dataset.name.toLowerCase();
    const status = row.dataset.status;

    const matchesSearch = name.includes(searchTerm);
    const matchesFilter = currentFilter === 'all' || status === currentFilter;

    if (matchesSearch && matchesFilter) {
      row.classList.remove('hidden');
      visibleCount++;
    } else {
      row.classList.add('hidden');
    }
  });

  emptyState.style.display = visibleCount === 0 ? 'block' : 'none';
}

function sortTable() {
  const rows = Array.from(tableBody.querySelectorAll('tr'));
  const column = currentSort.column;
  const direction = currentSort.direction;

  rows.sort((a, b) => {
    let aVal, bVal;

    if (column === 'name') {
      aVal = a.dataset.name.toLowerCase();
      bVal = b.dataset.name.toLowerCase();
      return direction === 'asc' ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal);
    } else {
      aVal = parseFloat(a.dataset[column]);
      bVal = parseFloat(b.dataset[column]);
      return direction === 'asc' ? aVal - bVal : bVal - aVal;
    }
  });

  rows.forEach((row) => tableBody.appendChild(row));
}

// Keyboard navigation for filter buttons
filterBtns.forEach((btn, index) => {
  btn.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      const nextBtn = filterBtns[index + 1] || filterBtns[0];
      nextBtn.focus();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      const prevBtn = filterBtns[index - 1] || filterBtns[filterBtns.length - 1];
      prevBtn.focus();
    }
  });
});
