// In frontend/src/App.tsx
import { ConfigProvider, Layout, theme, Typography } from 'antd';
import { Routes, Route } from 'react-router-dom';
import { ConnectionStatus } from './components/shared/ConnectionStatus';
import { AdminLayout } from './components/layouts/AdminLayout';
import { OptimizationsListPage } from './pages/OptimizationsListPage';
import { OptimizationDetailsPage } from './pages/OptimizationDetailsPage';
import { BacktestDetailsPage } from './pages/BacktestDetailsPage';
import { BacktestListPage } from './pages/BacktestListPage'; // Import the real page

// Placeholder pages - we will create these in the next tasks
const DashboardPage = () => <Typography.Title>Live Dashboard (Coming Soon)</Typography.Title>;

const NotFoundPage = () => <Typography.Title>404 - Page Not Found</Typography.Title>;

function App() {
  return (
    <ConfigProvider theme={{ algorithm: theme.darkAlgorithm }}>
      <ConnectionStatus />
      <Routes>
        {/* Main Dashboard Route */}
        <Route path="/" element={<DashboardPage />} />

        {/* Admin Panel Nested Routes */}
        <Route path="/admin" element={<AdminLayout />}>
          <Route path="optimizations" element={<OptimizationsListPage />} />
          <Route path="optimizations/:jobId" element={<OptimizationDetailsPage />} />
          <Route path="backtests" element={<BacktestListPage />} />
          <Route path="backtests/:runId" element={<BacktestDetailsPage />} /> {/* <-- Add this route */}
        </Route>
        
        {/* Catch-all 404 Route */}
        <Route path="*" element={<NotFoundPage />} />
      </Routes>
    </ConfigProvider>
  );
}

export default App;