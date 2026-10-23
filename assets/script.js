async function loadRankings() {
    const listElement = document.getElementById('rankings-list');

    try {
        const response = await fetch('/api/rankings');
        if (!response.ok) {
            throw new Error(`Server returned status: ${response.status}`);
        }

        const users = await response.json();
        listElement.innerHTML = '';

        if (!Array.isArray(users) || users.length === 0) {
            listElement.innerHTML = '<li>No rankings found.</li>';
            return;
        }

        users.forEach(({ name, level, image }) => {
            const li = document.createElement('li');
            li.className = 'user-card';

            const img = document.createElement('img');
    
            // Pass the raw 42 CDN URL through your backend proxy
            img.src = image ? `/api/avatar?url=${encodeURIComponent(image)}` : '';
            img.alt = `${name}'s avatar`;
            img.className = 'user-avatar';

            img.onerror = () => {
                img.src = `https://ui-avatars.com/api/?name=${encodeURIComponent(name)}`;
            };

            const nameSpan = document.createElement('span');
            nameSpan.className = 'user-name';
            nameSpan.textContent = name;

            const levelSpan = document.createElement('span');
            levelSpan.className = 'user-level';
            levelSpan.textContent = `Lvl ${level}`;

            li.appendChild(img);
            li.appendChild(nameSpan);
            li.appendChild(levelSpan);

            listElement.appendChild(li);
        });
    } catch (error) {
        console.error('Failed to fetch rankings:', error);
        listElement.innerHTML = '<li>Error loading rankings.</li>';
    }
}

document.addEventListener('DOMContentLoaded', loadRankings);
