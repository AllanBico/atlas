// In frontend/src/components/layouts/AdminLayout.tsx
import { Layout, Menu, Typography } from 'antd';
import { ExperimentOutlined, LineChartOutlined } from '@ant-design/icons';
import { Link, Outlet, useLocation } from 'react-router-dom';

const { Header, Content } = Layout;
const { Title } = Typography;

export const AdminLayout = () => {
  const location = useLocation();

  return (
    <Layout>
      <Header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 24px' }}>
        <Link to="/">
          <Title level={3} style={{ color: 'white', margin: 0 }}>Atlas Admin</Title>
        </Link>
        <Menu
          theme="dark"
          mode="horizontal"
          selectedKeys={[location.pathname]}
          style={{ flex: 1, minWidth: 0, justifyContent: 'flex-end' }}
          items={[
            {
              key: '/admin/optimizations',
              icon: <ExperimentOutlined />,
              label: <Link to="/admin/optimizations">Optimizations</Link>,
            },
            {
              key: '/admin/backtests',
              icon: <LineChartOutlined />,
              label: <Link to="/admin/backtests">Backtest Runs</Link>,
            },
            // Add more admin pages here in the future
          ]}
        />
      </Header>
      <Content style={{ padding: '24px' }}>
        {/* The nested route components will be rendered here */}
        <Outlet />
      </Content>
    </Layout>
  );
};