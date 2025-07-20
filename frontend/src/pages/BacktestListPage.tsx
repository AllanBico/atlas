// In frontend/src/pages/BacktestListPage.tsx

import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Table, Typography, Alert, Spin, Button, Input, Space, Tag } from 'antd';
import { Link } from 'react-router-dom';
import { format } from 'date-fns';

const { Title, Text } = Typography; // Import Text for coloring
const { Search } = Input;

// Update the interface to include the new metrics
interface BacktestRun {
  id: number;
  strategy_name: string;
  symbol: string;
  interval: string;
  start_date: string;
  end_date: string;
  net_pnl_percentage?: number | null;
  total_trades?: number | null;
  sharpe_ratio?: number | null;
  max_drawdown_percentage?: number | null;
}

// Define the type for the paginated API response
interface PaginatedResponse {
  items: BacktestRun[];
  total_items: number;
  page: number;
  page_size: number;
}

const fetchBacktestRuns = async (page = 1, pageSize = 10, jobId?: number): Promise<PaginatedResponse> => {
  const params = new URLSearchParams({
    page: page.toString(),
    pageSize: pageSize.toString(),
  });
  
  if (jobId) {
    params.append('job_id', jobId.toString());
  }
  
  const response = await fetch(`http://localhost:8080/api/backtest-runs?${params}`);
  if (!response.ok) {
    throw new Error('Network response was not ok');
  }
  return response.json();
};

export const BacktestListPage = () => {
  const [pagination, setPagination] = useState({ page: 1, pageSize: 10 });
  const [jobIdFilter, setJobIdFilter] = useState<number | undefined>(undefined);

  const { data, isLoading, error } = useQuery<PaginatedResponse>({
    queryKey: ['backtestRuns', pagination.page, pagination.pageSize, jobIdFilter],
    queryFn: () => fetchBacktestRuns(pagination.page, pagination.pageSize, jobIdFilter),
    placeholderData: (previousData) => previousData
  });

  const handleTableChange = (pagination: any) => {
    setPagination({
      page: pagination.current,
      pageSize: pagination.pageSize,
    });
  };

  const handleJobIdSearch = (value: string) => {
    const jobId = value.trim() ? parseInt(value.trim()) : undefined;
    setJobIdFilter(jobId);
    setPagination({ page: 1, pageSize: pagination.pageSize }); // Reset to first page
  };
  
  // Update the columns definition
  const columns = [
    { title: 'Run ID', dataIndex: 'id', key: 'id', sorter: (a: BacktestRun, b: BacktestRun) => a.id - b.id },
    { title: 'Strategy', dataIndex: 'strategy_name', key: 'strategy_name' },
    { title: 'Symbol', dataIndex: 'symbol', key: 'symbol', render: (s: string) => <Tag>{s}</Tag> },
    
    // --- NEW METRIC COLUMNS ---
    {
      title: 'P&L (%)',
      dataIndex: 'net_pnl_percentage',
      key: 'pnl',
      render: (pnl: number | null | undefined) => {
        if (pnl == null) return <Text type="secondary">-</Text>;
        return <Text type={pnl > 0 ? 'success' : 'danger'}>{pnl.toFixed(2)}%</Text>;
      },
      sorter: (a: BacktestRun, b: BacktestRun) => (a.net_pnl_percentage ?? 0) - (b.net_pnl_percentage ?? 0),
    },
    {
      title: 'Max DD (%)',
      dataIndex: 'max_drawdown_percentage',
      key: 'drawdown',
      render: (dd: number | null | undefined) => {
        if (dd == null) return <Text type="secondary">-</Text>;
        return `${dd.toFixed(2)}%`;
      },
      sorter: (a: BacktestRun, b: BacktestRun) => (a.max_drawdown_percentage ?? 0) - (b.max_drawdown_percentage ?? 0),
    },
    {
      title: 'Sharpe',
      dataIndex: 'sharpe_ratio',
      key: 'sharpe',
      render: (sharpe: number | null | undefined) => {
        if (sharpe == null) return <Text type="secondary">-</Text>;
        return sharpe.toFixed(2);
      },
      sorter: (a: BacktestRun, b: BacktestRun) => (a.sharpe_ratio ?? 0) - (b.sharpe_ratio ?? 0),
    },
    {
      title: 'Trades',
      dataIndex: 'total_trades',
      key: 'trades',
      render: (trades: number | null | undefined) => {
        if (trades == null) return <Text type="secondary">-</Text>;
        return trades;
      },
      sorter: (a: BacktestRun, b: BacktestRun) => (a.total_trades ?? 0) - (b.total_trades ?? 0),
    },
    // --- END NEW METRIC COLUMNS ---

    { title: 'Date Range', key: 'date_range', render: (_: any, r: BacktestRun) => `${format(new Date(r.start_date), 'MM/dd/yy')} - ${format(new Date(r.end_date), 'MM/dd/yy')}` },
    { title: 'Action', key: 'action', render: (_: any, r: BacktestRun) => <Link to={`/admin/backtests/${r.id}`}><Button>View Details</Button></Link>},
  ];

  if (isLoading && !data) return <Spin tip="Loading Backtest Runs..." size="large" />;
  if (error) return <Alert message="Error" description={error.message} type="error" showIcon />;

  return (
    <div>
      <Title level={2}>Backtest Runs</Title>
      
      <Space style={{ marginBottom: 16 }}>
        <Search
          placeholder="Filter by Job ID (optional)"
          allowClear
          onSearch={handleJobIdSearch}
          style={{ width: 250 }}
        />
        {jobIdFilter && (
          <span>Filtering by Job ID: {jobIdFilter}</span>
        )}
      </Space>
      
      <Table
        columns={columns}
        dataSource={data?.items || []}
        rowKey="id"
        loading={isLoading}
        pagination={{
          current: pagination.page,
          pageSize: pagination.pageSize,
          total: data?.total_items || 0,
        }}
        onChange={handleTableChange}
      />
    </div>
  );
};