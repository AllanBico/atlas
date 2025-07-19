// In frontend/src/App.tsx
import { ConfigProvider, Layout, theme } from 'antd';
import { ConnectionStatus } from './components/shared/ConnectionStatus';

const { Content } = Layout;

function App() {
  return (
    <ConfigProvider
      theme={{
        algorithm: theme.darkAlgorithm,
      }}
    >
      <Layout style={{ minHeight: '100vh' }}>
        {/* Add the ConnectionStatus component here */}
        <ConnectionStatus />

        <Content style={{ padding: '48px' }}>
          <h1>Welcome to Atlas</h1>
          <p>The user interface is under construction.</p>
        </Content>
      </Layout>
    </ConfigProvider>
  );
}

export default App;