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

            // User Avatar
            const img = document.createElement('img');
            img.src = image;
            img.alt = `${name}'s avatar`;
            img.className = 'user-avatar';

            // 1. Tell browser not to send 'Referer: http://localhost:8080/'
            img.referrerPolicy = 'no-referrer';

            // 3. Fallback image if link is broken or null
            img.onerror = () => {
                img.src = 'https://ui-avatars.com/api/?name=' + encodeURIComponent(name);
            };

            // User Name
            const nameSpan = document.createElement('span');
            nameSpan.className = 'user-name';
            nameSpan.textContent = name;

            // User Level
            const levelSpan = document.createElement('span');
            levelSpan.className = 'user-level';
            levelSpan.textContent = `Lvl ${level}`;

            // Append elements
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
