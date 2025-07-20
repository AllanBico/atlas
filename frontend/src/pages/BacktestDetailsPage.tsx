// In frontend/src/pages/BacktestDetailsPage.tsx

import { useState } from 'react';
import { useQueries, useQuery } from '@tanstack/react-query';
import { useParams } from 'react-router-dom';
import { Table, Typography, Alert, Spin, Statistic, Row, Col, Card, Tag, Divider, Space } from 'antd';
import { Line } from '@ant-design/charts';
import { format } from 'date-fns';
import { 
  DollarOutlined, 
  PercentageOutlined, 
  TrophyOutlined, 
  RiseOutlined,
  ClockCircleOutlined,
  BarChartOutlined,
  SafetyOutlined,
  FireOutlined
} from '@ant-design/icons';
import type { ApiTrade, EquityPoint, FullPerformanceReport } from '../types';

const { Title, Text } = Typography;

// --- API Fetcher Functions ---
const fetchBacktestDetails = async (runId: string): Promise<FullPerformanceReport> => {
  const res = await fetch(`http://localhost:8080/api/backtests/${runId}`);
  if (!res.ok) throw new Error(`Failed to fetch backtest details for run ID ${runId}`);
  return res.json();
};

const fetchEquityCurve = async (runId: string): Promise<EquityPoint[]> => {
  const res = await fetch(`http://localhost:8080/api/backtests/${runId}/equity-curve`);
  if (!res.ok) throw new Error('Failed to fetch equity curve');
  return res.json();
};

const fetchTrades = async (runId: string, page = 1, pageSize = 20) => {
  const res = await fetch(`http://localhost:8080/api/backtests/${runId}/trades?page=${page}&pageSize=${pageSize}`);
  if (!res.ok) throw new Error('Failed to fetch trades');
  return res.json();
};

// --- Enhanced Statistic Card Component ---
interface StatCardProps {
  title: string;
  value: number;
  precision?: number;
  suffix?: string;
  color?: string;
  prefix?: string;
  icon?: React.ReactNode;
  description?: string;
}

const StatCard = ({ 
  title, 
  value, 
  precision = 2, 
  suffix = '', 
  color, 
  prefix = '', 
  icon,
  description 
}: StatCardProps) => (
  <Card 
    size="small" 
    style={{ height: '100%' }}
    bodyStyle={{ padding: '16px' }}
  >
    <Statistic 
      title={
        <Space>
          {icon}
          <span>{title}</span>
        </Space>
      }
      value={value} 
      precision={precision} 
      suffix={suffix} 
      prefix={prefix} 
      valueStyle={{ 
        color: color || (value >= 0 ? '#52c41a' : '#f5222d'),
        fontSize: '24px',
        fontWeight: 'bold'
      }}
    />
    {description && (
      <Text type="secondary" style={{ fontSize: '12px', display: 'block', marginTop: '4px' }}>
        {description}
      </Text>
    )}
  </Card>
);

// --- Helper function to format duration ---
const formatDuration = (seconds: number): string => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const days = Math.floor(hours / 24);
  
  if (days > 0) {
    return `${days}d ${hours % 24}h`;
  } else if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else {
    return `${minutes}m`;
  }
};

export const BacktestDetailsPage = () => {
  const { runId } = useParams<{ runId: string }>();
  
  // --- Parallel Data Fetching ---
  const [detailsQuery, equityCurveQuery] = useQueries({
    queries: [
      { queryKey: ['backtestDetails', runId], queryFn: () => fetchBacktestDetails(runId!), enabled: !!runId },
      { queryKey: ['equityCurve', runId], queryFn: () => fetchEquityCurve(runId!), enabled: !!runId },
    ],
  });

  const [pagination, setPagination] = useState({ page: 1, pageSize: 20 });
  const tradesQuery = useQuery({
    queryKey: ['trades', runId, pagination],
    queryFn: () => fetchTrades(runId!, pagination.page, pagination.pageSize),
    placeholderData: (previousData) => previousData,
    enabled: !!runId
  });

  // --- Loading and Error States ---
  if (detailsQuery.isLoading || equityCurveQuery.isLoading) {
    return <Spin tip="Loading Backtest Report..." size="large" fullscreen />;
  }
  if (detailsQuery.error || equityCurveQuery.error || tradesQuery.error) {
    const error = detailsQuery.error || equityCurveQuery.error || tradesQuery.error;
    return <Alert message="Error loading data" description={(error as Error).message} type="error" showIcon />;
  }

  const report = detailsQuery.data as FullPerformanceReport;
  const equityCurveData = (equityCurveQuery.data as EquityPoint[]).map(p => ({
    time: format(new Date(p.timestamp), 'yyyy-MM-dd HH:mm'),
    equity: parseFloat(p.value),
  }));

  // --- Chart Configuration ---
  const chartConfig = {
    data: equityCurveData,
    xField: 'time',
    yField: 'equity',
    xAxis: { title: { text: 'Time' } },
    yAxis: { title: { text: 'Portfolio Value ($)' } },
    smooth: true,
    height: 400,
    tooltip: {
      title: (t: string) => t,
      formatter: (datum: any) => ({ name: 'Equity', value: `$${datum.equity.toFixed(2)}` }),
    },
    theme: 'dark',
  };

  // --- Trades Table Columns ---
  const tradeColumns = [
    { title: 'Side', dataIndex: 'side', key: 'side', render: (side: string) => <Tag color={side === 'Long' ? '#52c41a' : '#f5222d'}>{side}</Tag>},
    { title: 'Entry Time', dataIndex: 'entry_time', key: 'entry_time', render: (t: string) => format(new Date(t), 'MM/dd HH:mm:ss')},
    { title: 'Duration', key: 'duration', render: (_: any, r: ApiTrade) => formatDuration((new Date(r.exit_time).getTime() - new Date(r.entry_time).getTime()) / 1000) },
    { title: 'Entry Price', dataIndex: 'entry_price', key: 'entry_price', render: (p: string) => parseFloat(p).toFixed(2) },
    { title: 'Exit Price', dataIndex: 'exit_price', key: 'exit_price', render: (p: string) => parseFloat(p).toFixed(2) },
    { title: 'P&L ($)', dataIndex: 'pnl', key: 'pnl', render: (pnl: string) => <Text type={parseFloat(pnl) >= 0 ? 'success' : 'danger'}>{parseFloat(pnl).toFixed(2)}</Text>},
    { title: 'Confidence', dataIndex: 'signal_confidence', key: 'confidence', render: (c: number) => `${(c * 100).toFixed(0)}%`},
    { title: 'Leverage', dataIndex: 'leverage', key: 'leverage', render: (l: number) => `${l}x` },
  ];
  
  return (
    <div style={{ padding: '24px' }}>
      <Title level={2}>
        <TrophyOutlined style={{ marginRight: '8px', color: '#faad14' }} />
        Backtest Run #{runId}
      </Title>
      
      {/* Primary Performance Metrics */}
      <Card title="Primary Performance Metrics" style={{ marginBottom: '24px' }}>
        <Row gutter={[16, 16]}>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Net P&L" 
              value={report.net_pnl_percentage} 
              suffix="%" 
              icon={<DollarOutlined />}
              description="Total return on investment"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Absolute P&L" 
              value={parseFloat(report.net_pnl_absolute)} 
              prefix="$" 
              icon={<DollarOutlined />}
              description="Total dollar gain/loss"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Max Drawdown" 
              value={report.max_drawdown_percentage} 
              suffix="%" 
              color="#f5222d"
              icon={<RiseOutlined />}
              description="Largest peak-to-trough decline"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Win Rate" 
              value={report.win_rate} 
              suffix="%" 
              color="#52c41a"
              icon={<BarChartOutlined />}
              description="Percentage of profitable trades"
            />
          </Col>
        </Row>
      </Card>

      {/* Risk & Return Metrics */}
      <Card title="Risk & Return Analysis" style={{ marginBottom: '24px' }}>
        <Row gutter={[16, 16]}>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Sharpe Ratio" 
              value={report.sharpe_ratio} 
              icon={<SafetyOutlined />}
              description="Risk-adjusted return metric"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Sortino Ratio" 
              value={report.sortino_ratio} 
              icon={<SafetyOutlined />}
              description="Downside risk-adjusted return"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Calmar Ratio" 
              value={report.calmar_ratio} 
              icon={<FireOutlined />}
              description="Return vs max drawdown ratio"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Profit Factor" 
              value={report.profit_factor} 
              icon={<RiseOutlined />}
              description="Gross profit / Gross loss"
            />
          </Col>
        </Row>
      </Card>

      {/* Trading Activity Metrics */}
      <Card title="Trading Activity" style={{ marginBottom: '24px' }}>
        <Row gutter={[16, 16]}>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Total Trades" 
              value={report.total_trades} 
              precision={0}
              icon={<BarChartOutlined />}
              description="Number of completed trades"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Avg Trade Duration" 
              value={report.avg_trade_duration_secs / 3600} 
              suffix=" hours"
              icon={<ClockCircleOutlined />}
              description="Average time per trade"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="Expectancy" 
              value={parseFloat(report.expectancy)} 
              prefix="$"
              icon={<DollarOutlined />}
              description="Average profit per trade"
            />
          </Col>
          <Col xs={24} sm={12} md={6}>
            <StatCard 
              title="LAROM" 
              value={report.larom} 
              icon={<PercentageOutlined />}
              description="Logarithmic return on margin"
            />
          </Col>
        </Row>
      </Card>

      {/* Additional Metrics */}
      <Card title="Additional Metrics" style={{ marginBottom: '24px' }}>
        <Row gutter={[16, 16]}>
          <Col xs={24} sm={12} md={8}>
            <StatCard 
              title="Funding P&L" 
              value={parseFloat(report.funding_pnl)} 
              prefix="$"
              icon={<DollarOutlined />}
              description="Funding rate impact"
            />
          </Col>
          <Col xs={24} sm={12} md={8}>
            <StatCard 
              title="Drawdown Duration" 
              value={report.drawdown_duration_secs / 86400} 
              suffix=" days"
              icon={<ClockCircleOutlined />}
              description="Time in max drawdown"
            />
          </Col>
          <Col xs={24} sm={12} md={8}>
            <StatCard 
              title="Absolute Drawdown" 
              value={parseFloat(report.max_drawdown_absolute)} 
              prefix="$"
              color="#f5222d"
              icon={<RiseOutlined />}
              description="Largest dollar drawdown"
            />
          </Col>
        </Row>
      </Card>

      {/* Equity Curve */}
      <Card title="Portfolio Equity Curve" style={{ marginBottom: '24px' }}>
        <Line {...chartConfig} />
      </Card>

      {/* Trade History */}
      <Card title="Trade History">
        <Table
          columns={tradeColumns}
          dataSource={(tradesQuery.data as any)?.items || []}
          rowKey={(_, index) => index!}
          loading={tradesQuery.isLoading}
          pagination={{
            current: pagination.page,
            pageSize: pagination.pageSize,
            total: (tradesQuery.data as any)?.total_items || 0,
            showSizeChanger: true,
            showQuickJumper: true,
            showTotal: (total, range) => `${range[0]}-${range[1]} of ${total} trades`,
          }}
          onChange={(p) => setPagination({ page: p.current!, pageSize: p.pageSize! })}
          scroll={{ x: 800 }}
          size="small"
        />
      </Card>
    </div>
  );
};