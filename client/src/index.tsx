/* @refresh reload */
import { render } from 'solid-js/web'
import { FactoriesProvider } from './FactoriesContext';
import { GameDatabaseProvider } from './GameDatabaseContext';
import App from './App'

const root = document.getElementById('root')
render(() => <GameDatabaseProvider>
    <FactoriesProvider>
        <App />
    </FactoriesProvider>
</GameDatabaseProvider>, root!)
